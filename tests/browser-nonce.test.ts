import { describe, it, expect } from 'vitest';
import { createNonce, isValidNonce } from '../browser/src/nonce';

describe('createNonce', () => {
  it('возвращает строку допустимого формата', () => {
    const nonce = createNonce();
    expect(nonce).not.toBeNull();
    if (nonce !== null) {
      expect(isValidNonce(nonce)).toBe(true);
    }
  });

  it('возвращает уникальные значения', () => {
    const seen = new Set<string>();
    for (let i = 0; i < 200; i += 1) {
      const nonce = createNonce();
      if (nonce === null) {
        expect.fail('nonce null');
      }
      expect(seen.has(nonce)).toBe(false);
      seen.add(nonce);
    }
  });
});

describe('isValidNonce', () => {
  it('принимает 16–64 символа [A-Za-z0-9_-]', () => {
    expect(isValidNonce('a'.repeat(16))).toBe(true);
    expect(isValidNonce('a'.repeat(64))).toBe(true);
    expect(isValidNonce('abcDEF012_-xyz'.repeat(3))).toBe(true);
  });

  it('отклоняет короткие, длинные и с недопустимыми символами', () => {
    expect(isValidNonce('a'.repeat(15))).toBe(false);
    expect(isValidNonce('a'.repeat(65))).toBe(false);
    expect(isValidNonce('')).toBe(false);
    expect(isValidNonce('a'.repeat(16) + '!')).toBe(false);
    expect(isValidNonce('a'.repeat(16) + ' ')).toBe(false);
    expect(isValidNonce('a'.repeat(16) + 'ю')).toBe(false);
  });
});
