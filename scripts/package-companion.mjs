import { mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import process from 'node:process';

const root = resolve(import.meta.dirname, '..');
const extensionDir = resolve(root, 'browser', 'extension');
const outputDir = resolve(root, 'artifacts', 'browser-companion');

function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

rmSync(extensionDir, { recursive: true, force: true });
run(process.execPath, [resolve(root, 'browser', 'build.mjs')]);

const manifestPath = resolve(extensionDir, 'manifest.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
const packageVersion = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8')).version;
if (manifest.version !== packageVersion) {
  throw new Error(
    `Browser manifest version ${manifest.version} does not match package version ${packageVersion}.`,
  );
}

for (const required of ['manifest.json', 'background.js', 'content.js']) {
  const path = resolve(extensionDir, required);
  if (statSync(path).size <= 0) throw new Error(`Missing or empty extension payload: ${path}`);
}
if (readdirSync(extensionDir, { recursive: true }).some((entry) => `${entry}`.endsWith('.map'))) {
  throw new Error('Browser Companion release payload must not contain source maps.');
}

rmSync(outputDir, { recursive: true, force: true });
mkdirSync(outputDir, { recursive: true });
const archiveName = `SOUL-Browser-Companion-v${manifest.version}.zip`;
const archivePath = resolve(outputDir, archiveName);
rmSync(archivePath, { force: true });

if (process.platform === 'win32') {
  run('powershell.exe', [
    '-NoProfile',
    '-Command',
    `Compress-Archive -Path '${extensionDir.replaceAll("'", "''")}\\*' -DestinationPath '${archivePath.replaceAll("'", "''")}' -CompressionLevel Optimal -Force`,
  ]);
} else {
  run('zip', ['-X', '-r', archivePath, '.'], extensionDir);
}

const size = statSync(archivePath).size;
if (size <= 0) throw new Error(`Browser Companion archive is empty: ${archivePath}`);
const digest = createHash('sha256').update(readFileSync(archivePath)).digest('hex');
writeFileSync(resolve(outputDir, 'SHA256SUMS'), `${digest}  ${archiveName}\n`);
if (statSync(resolve(outputDir, 'SHA256SUMS')).size <= 0) {
  throw new Error('Browser Companion checksum file is empty.');
}
console.log(`Packaged ${archivePath} size=${size} sha256=${digest}`);
