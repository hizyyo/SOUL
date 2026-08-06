import { describe, it, expect } from 'vitest';
import {
  isTrustedSenderForRequest,
  isContextResponse,
  validateNativeResponse,
  validateOutgoingRequest,
} from '../browser/src/protocol';
import {
  CONTEXT_POLICY_VERSION,
  EXTENSION_ID,
  MAX_PACK_CHARS,
  MAX_TASK_CHARS,
  PROTOCOL_VERSION,
} from '../browser/src/constants';

const base = {
  protocol: PROTOCOL_VERSION,
  extensionId: EXTENSION_ID,
  nonce: 'n'.repeat(20),
};

const validContext = {
  type: 'soul.context',
  protocol: PROTOCOL_VERSION,
  nonce: base.nonce,
  ok: true,
  pack: '{"claims":[]}',
  entityCount: 3,
  tokenEstimate: 420,
  costEstimateUsd: 0.0021,
  policyVersion: CONTEXT_POLICY_VERSION,
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

  it('привязывает ответ к nonce и проверяет границы pack/token budget', () => {
    expect(isContextResponse(validContext, base.nonce)).toBe(true);
    expect(isContextResponse(validContext, 'x'.repeat(20))).toBe(false);
    expect(isContextResponse({ ...validContext, pack: '' })).toBe(false);
    expect(isContextResponse({ ...validContext, pack: 'x'.repeat(MAX_PACK_CHARS + 1) })).toBe(false);
    expect(isContextResponse({ ...validContext, tokenEstimate: 901 })).toBe(false);
    expect(isContextResponse({ ...validContext, maxTokens: 3001 })).toBe(false);
    expect(isContextResponse({ ...validContext, unexpected: true })).toBe(false);
  });

  it('strictly validates native success and error responses', () => {
    expect(validateNativeResponse(validContext, base.nonce)).toEqual(validContext);
    expect(
      validateNativeResponse(
        {
          type: 'soul.error',
          protocol: PROTOCOL_VERSION,
          nonce: base.nonce,
          ok: false,
          code: 'runtime_error',
          message: 'failed',
        },
        base.nonce,
      ),
    ).not.toBeNull();
    expect(validateNativeResponse({ ...validContext, ok: false }, base.nonce)).toBeNull();
  });
});

describe('sender origin binding', () => {
  const contextRequest = {
    type: 'soul.get_context' as const,
    ...base,
    origin: 'https://chatgpt.com',
    task: 'question',
    maxTokens: 900,
  };

  it('requires the sender URL origin to match a context request', () => {
    expect(
      isTrustedSenderForRequest(
        { id: EXTENSION_ID, url: 'https://chatgpt.com/c/123' },
        contextRequest,
      ),
    ).toBe(true);
    expect(
      isTrustedSenderForRequest(
        { id: EXTENSION_ID, url: 'https://claude.ai/chat/123' },
        contextRequest,
      ),
    ).toBe(false);
    expect(
      isTrustedSenderForRequest(
        { id: EXTENSION_ID, url: 'chrome-extension://extension/page.html' },
        contextRequest,
      ),
    ).toBe(false);
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
