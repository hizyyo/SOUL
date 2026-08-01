// Создаёт/читает фиксированный RSA-ключ расширения MV3 и вычисляет его ID.
//
// Chrome вычисляет ID расширения из ключа: первые 16 байт SHA-256 от
// DER SubjectPublicKeyInfo, каждый байт → две буквы алфавита a-p
// (старшая половина, младшая половина).
//
// Ключ сохраняется в browser/keys/extension-key.base64 (одна строка) и
// переиспользуется при повторных запусках — ID расширения стабилен.
//
// Запуск: node browser/scripts/make-extension-id.mjs
// Печатает: base64(DER SPKI) для поля "key" и стабильный ID расширения.

import { createHash, generateKeyPairSync } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const keyPath = join(root, '..', 'keys', 'extension-key.base64');

let keyB64;
try {
  keyB64 = (await readFile(keyPath, 'utf8')).trim();
  if (!/^[A-Za-z0-9+/=]+$/.test(keyB64)) {
    throw new Error('некорректный ключ');
  }
} catch {
  const { publicKey } = generateKeyPairSync('rsa', {
    modulusLength: 2048,
    publicExponent: 0x10001,
  });
  const der = publicKey.export({ type: 'spki', format: 'der' });
  keyB64 = der.toString('base64');
  await mkdir(dirname(keyPath), { recursive: true });
  await writeFile(keyPath, `${keyB64}\n`, 'utf8');
}

const der = Buffer.from(keyB64, 'base64');
const digest = createHash('sha256').update(der).digest();
const id = [...digest.subarray(0, 16)]
  .flatMap((b) => [b >> 4, b & 0x0f])
  .map((nibble) => String.fromCharCode(0x61 + nibble))
  .join('');

console.log(`key (base64, 64 lines):`);
for (let i = 0; i < keyB64.length; i += 64) {
  console.log(keyB64.slice(i, i + 64));
}
console.log(`extension id: ${id}`);
