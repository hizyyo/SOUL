// Сборка расширения SOUL Browser Companion (Manifest V3).
//
// Использует programmatic API Vite (rolldown) из node_modules проекта —
// новых зависимостей не добавляет. Входные точки TypeScript, выходные файлы
// классических скриптов (IIFE) — контентные скрипты не могут быть ES-модулями.
//
// Запуск: pnpm build:companion
// Выход: browser/extension/ (background.js, content.js, manifest.json, icons/)

import { build } from 'vite';
import { copyFile, mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const outDir = join(root, 'extension');
const stagingDir = join(root, `.extension-build-${process.pid}-${Date.now()}`);
const backupDir = join(root, `.extension-backup-${process.pid}-${Date.now()}`);
const manifestSrc = join(root, 'manifest.source.json');

const entries = [
  { name: 'background', file: join(root, 'src', 'background.ts') },
  { name: 'content', file: join(root, 'src', 'content.ts') },
];

let backedUp = false;
try {
  await mkdir(stagingDir, { recursive: true });
  for (const entry of entries) {
    await build({
      logLevel: 'warn',
      build: {
        outDir: stagingDir,
        emptyOutDir: false,
        minify: false,
        sourcemap: false,
        rollupOptions: {
          input: entry.file,
          output: {
            format: 'iife',
            entryFileNames: `${entry.name}.js`,
          },
        },
      },
    });
  }

  const manifest = JSON.parse(await readFile(manifestSrc, 'utf8'));
  const iconsDir = join(root, 'icons');
  await mkdir(join(stagingDir, 'icons'), { recursive: true });
  for (const icon of Object.values(manifest.icons ?? {})) {
    const name = icon.replace(/^icons\//, '');
    await copyFile(join(iconsDir, name), join(stagingDir, 'icons', name));
  }
  await writeFile(
    join(stagingDir, 'manifest.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );

  await rm(backupDir, { recursive: true, force: true });
  try {
    await rename(outDir, backupDir);
    backedUp = true;
  } catch (error) {
    if (error?.code !== 'ENOENT') {
      throw error;
    }
  }
  try {
    await rename(stagingDir, outDir);
  } catch (error) {
    if (backedUp) {
      await rename(backupDir, outDir);
      backedUp = false;
    }
    throw error;
  }
  if (backedUp) {
    await rm(backupDir, { recursive: true, force: true });
  }
} finally {
  await rm(stagingDir, { recursive: true, force: true });
}

console.log(`Browser Companion extension built into ${outDir}`);
