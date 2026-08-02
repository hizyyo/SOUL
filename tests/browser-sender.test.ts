import { describe, it, expect, beforeAll, vi } from 'vitest';
import { EXTENSION_ID, HOST_NAME, PROTOCOL_VERSION } from '../browser/src/constants';
import { isTrustedSender } from '../browser/src/protocol';

type Listener = (
  message: unknown,
  sender: { id?: string; url?: string },
  sendResponse: (response: unknown) => void,
) => unknown;

const listeners: Listener[] = [];
const connectNative = vi.fn();
const port = {
  name: HOST_NAME,
  postMessage: vi.fn(),
  onMessage: { addListener: vi.fn() },
  onDisconnect: { addListener: vi.fn() },
};

beforeAll(async () => {
  (globalThis as { window?: unknown }).window = globalThis;
  (globalThis as { chrome?: unknown }).chrome = {
    runtime: {
      id: EXTENSION_ID,
      sendMessage: vi.fn(),
      connectNative,
      lastError: undefined,
      onMessage: {
        addListener: (fn: Listener) => {
          listeners.push(fn);
        },
      },
    },
  };
  await import('../browser/src/background');
});

const validPing = {
  type: 'soul.ping',
  protocol: PROTOCOL_VERSION,
  extensionId: EXTENSION_ID,
  nonce: 'n'.repeat(20),
};

describe('isTrustedSender', () => {
  it('доверяет только собственному id расширения', () => {
    expect(isTrustedSender({ id: EXTENSION_ID })).toBe(true);
    expect(isTrustedSender({ id: 'other-extension-id' })).toBe(false);
    expect(isTrustedSender({ id: undefined })).toBe(false);
    expect(isTrustedSender(undefined)).toBe(false);
    expect(isTrustedSender(null)).toBe(false);
  });
});

describe('background onMessage sender validation', () => {
  it('отклоняет сообщение от другого расширения', () => {
    const respond = vi.fn();
    connectNative.mockClear();
    port.postMessage.mockClear();
    for (const fn of listeners) {
      fn(validPing, { id: 'other-extension-id', url: 'chrome-extension://other/page.html' }, respond);
    }
    expect(respond).toHaveBeenCalledOnce();
    const response = respond.mock.calls[0]?.[0] as { type: string; code: string };
    expect(response.type).toBe('soul.error');
    expect(response.code).toBe('invalid_sender');
    expect(connectNative).not.toHaveBeenCalled();
    expect(port.postMessage).not.toHaveBeenCalled();
  });

  it('отклоняет сообщение от веб-страницы (нет sender.id)', () => {
    const respond = vi.fn();
    connectNative.mockClear();
    for (const fn of listeners) {
      fn(validPing, { url: 'https://evil.example/page' }, respond);
    }
    const response = respond.mock.calls[0]?.[0] as { code: string };
    expect(response.code).toBe('invalid_sender');
    expect(connectNative).not.toHaveBeenCalled();
  });

  it('пропускает сообщение от собственного контент-скрипта', () => {
    const respond = vi.fn();
    connectNative.mockClear();
    port.postMessage.mockClear();
    connectNative.mockReturnValue(port);
    for (const fn of listeners) {
      fn(validPing, { id: EXTENSION_ID, url: 'https://chatgpt.com/page' }, respond);
    }
    expect(connectNative).toHaveBeenCalledOnce();
    expect(port.postMessage).toHaveBeenCalledOnce();
    const sent = port.postMessage.mock.calls[0]?.[0] as { type: string };
    expect(sent.type).toBe('soul.ping');
  });
});
