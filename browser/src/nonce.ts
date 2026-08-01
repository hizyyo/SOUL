/**
 * Nonce для запросов к Native Messaging host. Случайные значения из
 * crypto.getRandomValues, 24 байта → base64url (32 символа) — формат,
 * допустимый host-ом (16–64 символа [A-Za-z0-9_-]).
 */

const NONCE_BYTES = 24;

/** Создаёт новый случайный nonce. Возвращает null, если crypto недоступен. */
export function createNonce(): string | null {
  const cryptoObject = globalThis.crypto;
  if (!cryptoObject?.getRandomValues) {
    return null;
  }
  const bytes = new Uint8Array(NONCE_BYTES);
  cryptoObject.getRandomValues(bytes);
  let out = '';
  for (const byte of bytes) {
    out += String.fromCharCode(byte);
  }
  return btoa(out).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

/** Формат nonce: 16–64 символа [A-Za-z0-9_-] (правило host-а). */
export function isValidNonce(nonce: string): boolean {
  if (nonce.length < 16 || nonce.length > 64) {
    return false;
  }
  return /^[A-Za-z0-9_-]+$/.test(nonce);
}
