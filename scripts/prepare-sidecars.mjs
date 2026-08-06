import { copyFileSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import process from 'node:process';

const root = resolve(import.meta.dirname, '..');
const tauriDir = resolve(root, 'src-tauri');
const debug = process.argv.includes('--debug');
const profile = debug ? 'debug' : 'release';

function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? tauriDir,
    env: options.env ?? process.env,
    stdio: 'inherit',
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function sha256(path) {
  const file = statSync(path);
  if (!file.isFile() || file.size <= 0) {
    throw new Error(`Required sidecar is missing or empty: ${path}`);
  }
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

const targetTriple = argumentValue('--target') ?? process.env.SOUL_TARGET_TRIPLE;

if (!targetTriple) {
  throw new Error(
    'An explicit target triple is required: --target <triple> or SOUL_TARGET_TRIPLE.',
  );
}
if (!/^[A-Za-z0-9_.-]+$/u.test(targetTriple)) {
  throw new Error(`Invalid Rust target triple: ${targetTriple}`);
}

const extension = targetTriple.includes('-windows-') ? '.exe' : '';
const outputDir = resolve(tauriDir, 'binaries');
mkdirSync(outputDir, { recursive: true });

const cargoArgs = [
  'build',
  '--locked',
  '--target',
  targetTriple,
  ...(debug ? [] : ['--release']),
  '--bin',
  'soul-mcp',
  '--bin',
  'soul-bridge',
];

if (process.platform === 'win32') {
  const script = resolve(root, 'scripts', 'release-check.ps1');
  const powershellArgs = [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    script,
    '-BuildOnly',
    '-Target',
    targetTriple,
  ];
  if (debug) powershellArgs.push('-DebugBuild');
  run('powershell.exe', powershellArgs, { cwd: root });
} else {
  run('cargo', cargoArgs, {
    env: { ...process.env, SOUL_SIDECAR_BUILD: '1' },
  });
}

for (const name of ['soul-mcp', 'soul-bridge']) {
  const source = resolve(tauriDir, 'target', targetTriple, profile, `${name}${extension}`);
  const target = resolve(outputDir, `${name}-${targetTriple}${extension}`);
  const sourceSize = statSync(source).size;
  if (sourceSize <= 0) {
    throw new Error(`Refusing to prepare zero-byte sidecar: ${source}`);
  }
  const sourceHash = sha256(source);
  copyFileSync(source, target);
  const targetSize = statSync(target).size;
  const targetHash = sha256(target);
  if (targetSize !== sourceSize || targetSize <= 0 || targetHash !== sourceHash) {
    throw new Error(`Prepared sidecar size/hash mismatch: ${target}`);
  }
  console.log(`prepared ${target} size=${targetSize} sha256=${targetHash}`);
}

console.log(`Prepared ${profile} sidecars for ${targetTriple}.`);
