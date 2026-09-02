[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [switch]$BuildOnly,
    [switch]$DebugBuild,
    [switch]$SkipPackaging,
    [string]$Target
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not $Target) {
    throw 'An explicit Rust target triple is required. Pass -Target x86_64-pc-windows-msvc.'
}
if ($Target -notmatch '^[A-Za-z0-9_.-]+$') {
    throw "Invalid Rust target triple: $Target"
}
if ($Target -ne 'x86_64-pc-windows-msvc') {
    throw 'scripts/release-check.ps1 currently qualifies x86_64-pc-windows-msvc releases only.'
}
if ($DebugBuild -and -not $BuildOnly -and -not $SkipPackaging) {
    throw 'DebugBuild can only be used with -BuildOnly or -SkipPackaging; release installers must contain release sidecars.'
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'Visual Studio Build Tools with the MSVC C++ toolchain are required for release checks.'
}

$vsRoot = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath
if (-not $vsRoot) {
    throw 'No Visual Studio installation with Microsoft.VisualStudio.Component.VC.Tools.x86.x64 was found.'
}

$vsDevCmd = Join-Path $vsRoot 'Common7\Tools\VsDevCmd.bat'
$linker = Get-ChildItem -Path (Join-Path $vsRoot 'VC\Tools\MSVC') `
    -Directory | Sort-Object Name -Descending | ForEach-Object {
        $candidate = Join-Path $_.FullName 'bin\Hostx64\x64\link.exe'
        if (Test-Path -LiteralPath $candidate) { $candidate }
    } | Select-Object -First 1
if (-not $linker) {
    throw 'MSVC link.exe was not found under the Visual Studio Build Tools installation.'
}

$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$tauriDir = Join-Path $root 'src-tauri'
$sidecarDir = Join-Path $tauriDir 'binaries'
$profile = if ($DebugBuild) { 'debug' } else { 'release' }
$extension = '.exe'
$targetReleaseDir = Join-Path $tauriDir "target\$Target\$profile"

if (-not (Test-Path -LiteralPath $sidecarDir)) {
    New-Item -ItemType Directory -Path $sidecarDir | Out-Null
}

function Invoke-MsvcCommand([string]$Command) {
    # A user-level Cargo config can select LLVM-MinGW lld-link. The vendored
    # OpenSSL artifacts require the official MSVC linker, so isolate the
    # override to this command and run in VsDevCmd's environment.
    $wrapped = '"' + $vsDevCmd + '" -arch=x64 -host_arch=x64 >nul' +
        ' && set "CARGO_TARGET_' + $Target.ToUpperInvariant().Replace('-', '_') + '_LINKER=' + $linker + '"' +
        ' && ' + $Command
    Push-Location $tauriDir
    try {
        & cmd.exe /d /s /c $wrapped
        if ($LASTEXITCODE -ne 0) {
            throw "Command failed with exit code ${LASTEXITCODE}: $Command"
        }
    } finally {
        Pop-Location
    }
}

function Copy-ReleaseSidecars {
    foreach ($name in @('soul-mcp', 'soul-bridge')) {
        $source = Join-Path $targetReleaseDir "$name$extension"
        if (-not (Test-Path -LiteralPath $source)) {
            throw "Required E2E sidecar is missing: $source"
        }
        if ((Get-Item -LiteralPath $source).Length -le 0) {
            throw "Required E2E sidecar is empty: $source"
        }
        $destination = Join-Path $sidecarDir "$name-$Target$extension"
        Copy-Item -LiteralPath $source -Destination $destination -Force
        if ((Get-Item -LiteralPath $destination).Length -ne (Get-Item -LiteralPath $source).Length) {
            throw "Prepared sidecar size mismatch: $destination"
        }
    }
}

function Invoke-NsisSmoke {
    $nsisDir = Join-Path $tauriDir "target\$Target\release\bundle\nsis"
    $installer = Get-ChildItem -Path $nsisDir -Filter '*.exe' -File -ErrorAction SilentlyContinue |
        Sort-Object Length -Descending | Select-Object -First 1
    if (-not $installer -or $installer.Length -le 0) {
        throw "NSIS installer was not produced under $nsisDir."
    }

    $installDir = Join-Path ([System.IO.Path]::GetTempPath()) "soul-installer-smoke-$([guid]::NewGuid())"
    try {
        $process = Start-Process -FilePath $installer.FullName -ArgumentList @('/S', "/D=$installDir") -Wait -PassThru
        if ($process.ExitCode -ne 0) {
            throw "NSIS installer smoke failed with exit code $($process.ExitCode)."
        }
        $app = Get-ChildItem -Path $installDir -Filter 'SOUL.exe' -File -Recurse -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if (-not $app -or $app.Length -le 0) {
            throw "Installed SOUL.exe is missing or empty under $installDir."
        }
        $mcp = Get-ChildItem -Path $installDir -Filter 'soul-mcp*.exe' -File -Recurse -ErrorAction SilentlyContinue |
            Select-Object -First 1
        $bridge = Get-ChildItem -Path $installDir -Filter 'soul-bridge*.exe' -File -Recurse -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if (-not $mcp -or $mcp.Length -le 0) {
            throw 'Installed sidecar is missing or empty: soul-mcp*.exe'
        }
        if (-not $bridge -or $bridge.Length -le 0) {
            throw 'Installed sidecar is missing or empty: soul-bridge*.exe'
        }

        # Verify the actual installed binaries against the staged hashes and
        # execute their MCP/native-messaging smoke tests from the install tree.
        & node (Join-Path $PSScriptRoot 'verify-sidecars.mjs') `
            --target $Target `
            --mcp-path $mcp.FullName `
            --bridge-path $bridge.FullName `
            --source-dir $sidecarDir `
            --source-prepared
        if ($LASTEXITCODE -ne 0) {
            throw "Installed sidecar verification failed with exit code $LASTEXITCODE."
        }
        Write-Host "NSIS install smoke passed: $($installer.FullName) -> $installDir"
    } finally {
        if (Test-Path -LiteralPath $installDir) {
            Remove-Item -LiteralPath $installDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

$cargoProfile = if ($DebugBuild) { '' } else { '--release' }
$sidecarBuild = "set SOUL_SIDECAR_BUILD=1 && cargo build --locked --target $Target $cargoProfile --bin soul-mcp --bin soul-bridge"

if (-not $SkipBuild) {
    Invoke-MsvcCommand $sidecarBuild
    # Tauri's build script validates externalBin even for lib tests, so stage
    # the freshly-built sidecars before those tests on a clean checkout.
    Copy-ReleaseSidecars
}

if ($BuildOnly) {
    return
}

foreach ($name in @('soul-mcp', 'soul-bridge')) {
    $e2eBinary = Join-Path $targetReleaseDir "$name$extension"
    if (-not (Test-Path -LiteralPath $e2eBinary) -or (Get-Item -LiteralPath $e2eBinary).Length -le 0) {
        throw "Release E2E preflight failed: $e2eBinary is missing or empty."
    }
}

Invoke-MsvcCommand "cargo test --locked --release --target $Target --lib"
Invoke-MsvcCommand "cargo test --locked --release --target $Target --lib -- --ignored --nocapture"

# The lib tests rebuild the package without SOUL_SIDECAR_BUILD, so copy only
# after they finish to bind the prepared binaries to the final Cargo outputs.
Copy-ReleaseSidecars

& node (Join-Path $PSScriptRoot 'verify-sidecars.mjs') `
    --target $Target `
    --prepared-dir $sidecarDir `
    --source-dir $targetReleaseDir
if ($LASTEXITCODE -ne 0) {
    throw "Prepared sidecar verification failed with exit code $LASTEXITCODE."
}

if (-not $SkipPackaging) {
    $env:SOUL_TARGET_TRIPLE = $Target
    $env:SOUL_SKIP_SIDECAR_BUILD = '1'
    try {
        Invoke-MsvcCommand "set SOUL_TARGET_TRIPLE=$Target && pnpm tauri build --target $Target --no-bundle --ci --no-sign"

        # Tauri's Cargo build compiles every binary target and replaces the
        # sidecar outputs. Stage and verify those final binaries before bundling.
        Copy-ReleaseSidecars
        & node (Join-Path $PSScriptRoot 'verify-sidecars.mjs') `
            --target $Target `
            --prepared-dir $sidecarDir `
            --source-dir $targetReleaseDir
        if ($LASTEXITCODE -ne 0) {
            throw "Final sidecar verification failed with exit code $LASTEXITCODE."
        }

        Invoke-MsvcCommand "set SOUL_TARGET_TRIPLE=$Target && pnpm tauri bundle --target $Target --bundles nsis --ci --no-sign"
    } finally {
        Remove-Item Env:SOUL_TARGET_TRIPLE -ErrorAction SilentlyContinue
        Remove-Item Env:SOUL_SKIP_SIDECAR_BUILD -ErrorAction SilentlyContinue
    }

    & node (Join-Path $PSScriptRoot 'package-companion.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw "Browser Companion packaging failed with exit code $LASTEXITCODE."
    }

    & node (Join-Path $PSScriptRoot 'verify-release.mjs') `
        --target $Target `
        --app-dir (Join-Path $tauriDir "target\$Target\release")
    if ($LASTEXITCODE -ne 0) {
        throw "Bundled payload verification failed with exit code $LASTEXITCODE."
    }
    Invoke-NsisSmoke
}
