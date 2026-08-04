[CmdletBinding()]
param(
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

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

$tauriDir = Join-Path $PSScriptRoot '..\src-tauri'
$cargoSteps = if ($SkipBuild) {
    'cargo test --release --lib -- --ignored --nocapture'
} else {
    'cargo build --release --bins && cargo test --release --lib -- --ignored --nocapture'
}

# The user-level Cargo config can point the MSVC target at LLVM-MinGW lld-link.
# That linker is incompatible with the vendored OpenSSL artifacts here. Run every
# release check in the official MSVC environment and override only this process.
$command = '"' + $vsDevCmd + '" -arch=x64 -host_arch=x64 >nul && set "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=' + $linker + '" && ' + $cargoSteps
Push-Location $tauriDir
try {
    & cmd.exe /d /s /c $command
    if ($LASTEXITCODE -ne 0) {
        throw "Release check failed with exit code $LASTEXITCODE."
    }
} finally {
    Pop-Location
}
