/**
 * Service worker расширения: единственная точка контакта с Native Messaging
 * host-ом. Проверяет входящие запросы (fail-closed), прозрачно пробрасывает
 * их в host и маршрутизирует ответы обратно в content script.
 *
 * Расширение никогда не сохраняет контекст: ни chrome.storage, ни логи.
 */

import { HOST_NAME, HOST_TIMEOUT_MS } from './constants';
import {
  errorResponse,
  isErrorResponse,
  validateOutgoingRequest,
  type BridgeIncoming,
  type ErrorResponse,
  type OutgoingRequest,
} from './protocol';

interface PendingEntry {
  resolve(value: BridgeIncoming): void;
  timer: number;
}

let port: ChromeNativePort | null = null;
const pending = new Map<string, PendingEntry>();

function ensurePort(): ChromeNativePort | null {
  if (port) {
    return port;
  }
  try {
    const candidate = chrome.runtime.connectNative(HOST_NAME);
    candidate.onMessage.addListener(handleNativeMessage);
    candidate.onDisconnect.addListener(handleDisconnect);
    port = candidate;
  } catch {
    return null;
  }
  return port;
}

function handleNativeMessage(message: unknown): void {
  if (typeof message !== 'object' || message === null) {
    return;
  }
  const nonce = (message as { nonce?: unknown }).nonce;
  if (typeof nonce !== 'string') {
    return;
  }
  const entry = pending.get(nonce);
  if (!entry) {
    return;
  }
  pending.delete(nonce);
  window.clearTimeout(entry.timer);
  entry.resolve(message as BridgeIncoming);
}

function handleDisconnect(): void {
  port = null;
  for (const [nonce, entry] of pending) {
    pending.delete(nonce);
    window.clearTimeout(entry.timer);
    entry.resolve(errorResponse('runtime_error', 'Соединение с SOUL host-ом разорвано.'));
  }
}

function sendToHost(request: OutgoingRequest): Promise<BridgeIncoming> {
  return new Promise<BridgeIncoming>((resolve) => {
    const candidate = ensurePort();
    if (!candidate) {
      resolve(
        errorResponse(
          'runtime_error',
          `Native host "${HOST_NAME}" недоступен. Установите SOUL и включите Browser Companion в настройках.`,
        ),
      );
      return;
    }
    const timer = window.setTimeout(() => {
      pending.delete(request.nonce);
      resolve(errorResponse('runtime_error', 'Превышено время ожидания ответа host-а.'));
    }, HOST_TIMEOUT_MS);
    pending.set(request.nonce, { resolve, timer });
    candidate.postMessage(request);
  });
}

async function handleRequest(message: unknown): Promise<BridgeIncoming> {
  const validated = validateOutgoingRequest(message);
  if (!validated.ok) {
    return validated.error;
  }
  return sendToHost(validated.request);
}

chrome.runtime.onMessage.addListener(
  (message: unknown, _sender: ChromeSender, sendResponse: (response: unknown) => void) => {
    void handleRequest(message).then(sendResponse);
    return true;
  },
);

export type { BridgeIncoming, ErrorResponse };
export { isErrorResponse };
