import { readFileSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
import process from 'node:process';

function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function requireNonemptyFile(path, label) {
  const stat = statSync(path);
  if (!stat.isFile() || stat.size <= 0) {
    throw new Error(`${label} is missing or empty: ${path}`);
  }
  console.log(`verified ${label}: ${path} size=${stat.size} sha256=${sha256(path)}`);
  return { path, size: stat.size, sha256: sha256(path) };
}

const target = argumentValue('--target');
const appDir = argumentValue('--app-dir');
if (!target || !appDir) {
  throw new Error(
    'Usage: node scripts/verify-release.mjs --target <triple> --app-dir <target-profile-dir>',
  );
}

const extension = target.includes('-windows-') ? '.exe' : '';
requireNonemptyFile(resolve(appDir, `soul${extension}`), 'Tauri application');
