import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import process from 'node:process';

const root = resolve(import.meta.dirname, '..');

function run(command, args) {
  const result =
    process.platform === 'win32'
      ? spawnSync('cmd.exe', ['/d', '/s', '/c', command, ...args], { cwd: root, stdio: 'inherit' })
      : spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

run('pnpm', ['build']);

// Release qualification already prepared and verified these exact sidecars.
// Avoid recursively invoking release-check through Tauri's build hook.
if (process.env.SOUL_SKIP_SIDECAR_BUILD !== '1') {
  run('pnpm', ['build:sidecars']);
}
