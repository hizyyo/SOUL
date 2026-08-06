import { mkdtempSync, readFileSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import process from 'node:process';

function argumentValue(name) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : undefined;
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function assertBinary(path) {
  const stat = statSync(path);
  if (!stat.isFile()) throw new Error(`Sidecar is not a file: ${path}`);
  const size = stat.size;
  if (size <= 0) throw new Error(`Sidecar is empty: ${path}`);
  return { path, size, sha256: sha256(path) };
}

function nativeMessage(value) {
  const payload = Buffer.from(JSON.stringify(value));
  const frame = Buffer.allocUnsafe(payload.length + 4);
  frame.writeUInt32LE(payload.length, 0);
  payload.copy(frame, 4);
  return frame;
}

function verifyBridge(path) {
  const appDir = mkdtempSync(join(tmpdir(), 'soul-sidecar-bridge-'));
  try {
    const result = spawnSync(path, [], {
      env: { ...process.env, SOUL_APP_DIR: appDir },
      input: nativeMessage({
        type: 'soul.ping',
        protocol: 'soul-bridge/1',
        extensionId: 'epfbcmgajbpjbphepfbhcoibmoaflbld',
        nonce: 'release_verify_000000000000',
      }),
      timeout: 15_000,
    });
    if (result.error) throw result.error;
    if (result.status !== 0) {
      throw new Error(
        `${basename(path)} failed bridge smoke test (${result.status}): ${result.stderr.toString()}`,
      );
    }
    if (result.stdout.length < 4) {
      throw new Error(`${basename(path)} returned no native-messaging frame.`);
    }
    const length = result.stdout.readUInt32LE(0);
    const response = JSON.parse(result.stdout.subarray(4, 4 + length).toString('utf8'));
    if (response.type !== 'soul.pong') {
      throw new Error(
        `${basename(path)} returned unexpected response: ${JSON.stringify(response)}`,
      );
    }
  } finally {
    rmSync(appDir, { recursive: true, force: true });
  }
}

function verifyMcpStarts(path) {
  const appDir = mkdtempSync(join(tmpdir(), 'soul-sidecar-mcp-'));
  try {
    const env = { ...process.env, SOUL_APP_DIR: appDir };
    delete env.SOUL_MCP_CAPABILITY;
    const result = spawnSync(path, [], { env, input: '', timeout: 15_000 });
    if (result.error) throw result.error;
    const stderr = result.stderr.toString('utf8');
    if (result.status === 0 || !stderr.includes('soul-mcp:')) {
      throw new Error(
        `${basename(path)} did not produce the expected capability preflight rejection.`,
      );
    }
  } finally {
    rmSync(appDir, { recursive: true, force: true });
  }
}

function canExecuteTarget(target) {
  if (target.includes('-windows-')) {
    return (
      process.platform === 'win32' &&
      ((target.startsWith('x86_64-') && process.arch === 'x64') ||
        (target.startsWith('aarch64-') && process.arch === 'arm64'))
    );
  }
  if (target.includes('-apple-darwin')) {
    return (
      process.platform === 'darwin' &&
      ((target.startsWith('x86_64-') && process.arch === 'x64') ||
        (target.startsWith('aarch64-') && process.arch === 'arm64'))
    );
  }
  if (target.includes('-linux-')) {
    return (
      process.platform === 'linux' &&
      ((target.startsWith('x86_64-') && process.arch === 'x64') ||
        (target.startsWith('aarch64-') && process.arch === 'arm64'))
    );
  }
  return false;
}

export function verifySidecarPair({
  directory,
  sourceDirectory,
  target,
  bundled = false,
  mcpPath,
  bridgePath,
  sourcePrepared = false,
}) {
  const extension = target.includes('-windows-') ? '.exe' : '';
  const suffix = bundled ? '' : `-${target}`;
  const mcp = mcpPath ?? resolve(directory, `soul-mcp${suffix}${extension}`);
  const bridge = bridgePath ?? resolve(directory, `soul-bridge${suffix}${extension}`);
  const reports = [assertBinary(mcp), assertBinary(bridge)];
  if (sourceDirectory) {
    for (const report of reports) {
      const sourceName = basename(report.path).startsWith('soul-mcp')
        ? `soul-mcp${sourcePrepared ? `-${target}` : ''}${extension}`
        : `soul-bridge${sourcePrepared ? `-${target}` : ''}${extension}`;
      const source = assertBinary(resolve(sourceDirectory, sourceName));
      if (source.size !== report.size || source.sha256 !== report.sha256) {
        throw new Error(`Prepared sidecar does not match Cargo output: ${report.path}`);
      }
    }
  }
  if (canExecuteTarget(target)) {
    verifyMcpStarts(mcp);
    verifyBridge(bridge);
  } else {
    console.log(`Skipped sidecar execution smoke tests for foreign target ${target}.`);
  }
  for (const report of reports) {
    console.log(`verified ${report.path} size=${report.size} sha256=${report.sha256}`);
  }
  return reports;
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  const target = argumentValue('--target');
  const preparedDir = argumentValue('--prepared-dir');
  const bundledDir = argumentValue('--bundled-dir');
  const sourceDir = argumentValue('--source-dir');
  const mcpPath = argumentValue('--mcp-path');
  const bridgePath = argumentValue('--bridge-path');
  const sourcePrepared = process.argv.includes('--source-prepared');
  if (!target || (!preparedDir && !bundledDir && !(mcpPath && bridgePath))) {
    throw new Error(
      'Usage: node scripts/verify-sidecars.mjs --target <triple> (--prepared-dir <dir> | --bundled-dir <dir> | (--mcp-path <path> --bridge-path <path>))',
    );
  }
  verifySidecarPair({
    directory: preparedDir ?? bundledDir,
    sourceDirectory: sourceDir,
    target,
    bundled: Boolean(bundledDir),
    mcpPath,
    bridgePath,
    sourcePrepared,
  });
}
