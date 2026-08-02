import { describe, it, expect } from 'vitest';
import {
  isContextResponse,
  validateOutgoingRequest,
} from '../browser/src/protocol';
import { PROTOCOL_VERSION, EXTENSION_ID, MAX_TASK_CHARS } from '../browser/src/constants';

const base = {
  protocol: PROTOCOL_VERSION,
  extensionId: EXTENSION_ID,
  nonce: 'n'.repeat(20),
};

const validContext = {
  type: 'soul.context',
  pack: '{"claims":[]}',
  entityCount: 3,
  tokenEstimate: 420,
  policyVersion: '7f3a9c2e',
  stateVersion: '5b38f537',
  maxTokens: 900,
};

describe('isContextResponse', () => {
  it('принимает корректный soul.context', () => {
    expect(isContextResponse(validContext)).toBe(true);
  });

  it('отклоняет числовые версии вместо строк (несоответствие host-у)', () => {
    expect(
      isContextResponse({ ...validContext, policyVersion: 123 }),
    ).toBe(false);
    expect(
      isContextResponse({ ...validContext, stateVersion: 42 }),
    ).toBe(false);
  });

  it('отклоняет отсутствующие обязательные поля', () => {
    const { pack: _pack, ...noPack } = validContext;
    expect(isContextResponse(noPack)).toBe(false);
    expect(isContextResponse({ ...validContext, entityCount: '3' })).toBe(false);
    expect(isContextResponse(null)).toBe(false);
  });
});

describe('validateOutgoingRequest', () => {
  it('принимает корректный soul.ping', () => {
    const result = validateOutgoingRequest({ type: 'soul.ping', ...base });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.request.type).toBe('soul.ping');
    }
  });

  it('принимает корректный soul.get_context', () => {
    const result = validateOutgoingRequest({
      type: 'soul.get_context',
      ...base,
      origin: 'https://gemini.google.com',
      task: 'кратко',
      maxTokens: 900,
    });
    expect(result.ok).toBe(true);
  });

  it('отклоняет не-объект', () => {
    expect(validateOutgoingRequest('ping').ok).toBe(false);
    expect(validateOutgoingRequest(null).ok).toBe(false);
    expect(validateOutgoingRequest(42).ok).toBe(false);
  });

  it('отклоняет неверный протокол', () => {
    const result = validateOutgoingRequest({ type: 'soul.ping', ...base, protocol: 'evil/2' });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('invalid_protocol');
    }
  });

  it('отклоняет неверный extension ID', () => {
    const result = validateOutgoingRequest({ type: 'soul.ping', ...base, extensionId: 'other' });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('invalid_extension_id');
    }
  });

  it('отклоняет короткий, длинный и недопустимый nonce', () => {
    const short = validateOutgoingRequest({ type: 'soul.ping', ...base, nonce: 'x' });
    expect(short.ok).toBe(false);
    const long = validateOutgoingRequest({ type: 'soul.ping', ...base, nonce: 'x'.repeat(65) });
    expect(long.ok).toBe(false);
    const bad = validateOutgoingRequest({ type: 'soul.ping', ...base, nonce: 'a'.repeat(16) + '!' });
    expect(bad.ok).toBe(false);
    if (!bad.ok) {
      expect(bad.error.code).toBe('invalid_nonce');
    }
  });

  it('отклоняет неподдерживаемое происхождение', () => {
    const result = validateOutgoingRequest({
      type: 'soul.get_context',
      ...base,
      origin: 'https://evil.example',
      task: '',
      maxTokens: 900,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('invalid_origin');
    }
  });

  it('отклоняет слишком длинную задачу', () => {
    const result = validateOutgoingRequest({
      type: 'soul.get_context',
      ...base,
      origin: 'https://chatgpt.com',
      task: 't'.repeat(MAX_TASK_CHARS + 1),
      maxTokens: 900,
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('task_too_long');
    }
  });

  it('отклоняет maxTokens вне диапазона и нецелые', () => {
    for (const maxTokens of [0, -1, 3001, 1.5, Number.NaN, '900']) {
      const result = validateOutgoingRequest({
        type: 'soul.get_context',
        ...base,
        origin: 'https://chatgpt.com',
        task: '',
        maxTokens,
      });
      expect(result.ok).toBe(false);
    }
  });

  it('отклоняет неизвестный тип запроса', () => {
    const result = validateOutgoingRequest({ type: 'soul.exfiltrate', ...base });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('unsupported_request');
    }
  });

  it('отклоняет запрос, превышающий лимит кадра', () => {
    const result = validateOutgoingRequest({
      type: 'soul.get_context',
      ...base,
      origin: 'https://claude.ai',
      task: 't'.repeat(MAX_TASK_CHARS),
      maxTokens: 3000,
      extra: 'x'.repeat(2_000_000),
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('request_too_large');
    }
  });
});
