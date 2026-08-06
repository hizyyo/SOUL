/**
 * Протокол soul-bridge/1: типы запросов/ответов и валидация исходящих
 * запросов. Зеркалирует src-tauri/src/bridge.rs (коды ошибок и лимиты).
 */

import {
  CONTEXT_POLICY_VERSION,
  EXTENSION_ID,
  MAX_FRAME_BYTES,
  MAX_PACK_CHARS,
  MAX_TASK_CHARS,
  MAX_TOKENS,
  PROTOCOL_VERSION,
  SUPPORTED_ORIGINS,
} from './constants';
import { isValidNonce } from './nonce';

export interface PingRequest {
  type: 'soul.ping';
  protocol: string;
  extensionId: string;
  nonce: string;
}

export interface GetContextRequest {
  type: 'soul.get_context';
  protocol: string;
  extensionId: string;
  nonce: string;
  origin: string;
  task: string;
  maxTokens: number;
}

export type OutgoingRequest = PingRequest | GetContextRequest;

export interface PongResponse {
  type: 'soul.pong';
  protocol: string;
  nonce: string;
  ok: true;
}

export interface ContextResponse {
  type: 'soul.context';
  protocol: string;
  nonce: string;
  ok: true;
  pack: string;
  entityCount: number;
  tokenEstimate: number;
  costEstimateUsd: number;
  /** Версия policy-кодекса host-а. */
  policyVersion: string;
  /** 8 hex-символов: хэш состояния БД (host шлёт строкой, см. context.rs). */
  stateVersion: string;
  maxTokens: number;
}

export interface ErrorResponse {
  type: 'soul.error';
  code: string;
  message: string;
  protocol?: string;
  nonce?: string | null;
  ok?: false;
}

export type BridgeIncoming = PongResponse | ContextResponse | ErrorResponse;

export function errorResponse(code: string, message: string): ErrorResponse {
  return { type: 'soul.error', code, message };
}

export function isErrorResponse(value: unknown): value is ErrorResponse {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { type?: unknown }).type === 'soul.error' &&
    typeof (value as { code?: unknown }).code === 'string' &&
    typeof (value as { message?: unknown }).message === 'string'
  );
}

export function isContextResponse(
  value: unknown,
  expectedNonce?: string,
): value is ContextResponse {
  if (!isRecord(value) || value.type !== 'soul.context') {
    return false;
  }
  const nonce = value.nonce;
  const pack = value.pack;
  const entityCount = value.entityCount;
  const tokenEstimate = value.tokenEstimate;
  const maxTokens = value.maxTokens;
  const costEstimateUsd = value.costEstimateUsd;
  return (
    hasOnlyKeys(value, [
      'type',
      'protocol',
      'nonce',
      'ok',
      'pack',
      'entityCount',
      'tokenEstimate',
      'costEstimateUsd',
      'policyVersion',
      'stateVersion',
      'maxTokens',
    ]) &&
    value.protocol === PROTOCOL_VERSION &&
    value.ok === true &&
    typeof nonce === 'string' &&
    isValidNonce(nonce) &&
    (expectedNonce === undefined || nonce === expectedNonce) &&
    typeof pack === 'string' &&
    characterCount(pack) > 0 &&
    characterCount(pack) <= MAX_PACK_CHARS &&
    utf8ByteLength(pack) <= MAX_FRAME_BYTES &&
    isIntegerInRange(entityCount, 0, MAX_PACK_CHARS) &&
    isIntegerInRange(maxTokens, 1, MAX_TOKENS) &&
    isIntegerInRange(tokenEstimate, 0, maxTokens) &&
    typeof costEstimateUsd === 'number' &&
    Number.isFinite(costEstimateUsd) &&
    costEstimateUsd >= 0 &&
    costEstimateUsd <= MAX_TOKENS &&
    value.policyVersion === CONTEXT_POLICY_VERSION &&
    typeof value.stateVersion === 'string' &&
    /^[0-9a-f]{8}$/.test(value.stateVersion) &&
    serializedByteLength(value) <= MAX_FRAME_BYTES
  );
}

/** Валидирует ответ native host-а и привязывает его к ожидаемому nonce. */
export function validateNativeResponse(
  value: unknown,
  expectedNonce: string,
): BridgeIncoming | null {
  if (isContextResponse(value, expectedNonce)) {
    return value;
  }
  if (!isRecord(value) || value.protocol !== PROTOCOL_VERSION || value.nonce !== expectedNonce) {
    return null;
  }
  if (!isValidNonce(expectedNonce) || serializedByteLength(value) > MAX_FRAME_BYTES) {
    return null;
  }
  if (
    value.type === 'soul.pong' &&
    value.ok === true &&
    hasOnlyKeys(value, ['type', 'protocol', 'nonce', 'ok'])
  ) {
    return value as unknown as PongResponse;
  }
  if (
    value.type === 'soul.error' &&
    value.ok === false &&
    hasOnlyKeys(value, ['type', 'protocol', 'nonce', 'ok', 'code', 'message']) &&
    typeof value.code === 'string' &&
    value.code.length > 0 &&
    value.code.length <= 64 &&
    typeof value.message === 'string' &&
    characterCount(value.message) <= 2000
  ) {
    return value as unknown as ErrorResponse;
  }
  return null;
}

export type ValidationResult =
  { ok: true; request: OutgoingRequest } | { ok: false; error: ErrorResponse };

/**
 * Доверенным считается только сообщение, пришедшее от собственного
 * контент-скрипта: chrome.runtime гарантирует, что sender.id нельзя
 * подделать (в отличие от полей extensionId/origin внутри сообщения).
 */
export function isTrustedSender(sender: { readonly id?: unknown } | undefined | null): boolean {
  return sender?.id === EXTENSION_ID;
}

/** Привязывает context-запрос к реальной странице, вызвавшей content script. */
export function isTrustedSenderForRequest(
  sender:
    | { readonly id?: unknown; readonly url?: unknown; readonly origin?: unknown }
    | undefined
    | null,
  request: OutgoingRequest,
): boolean {
  if (!isTrustedSender(sender)) {
    return false;
  }
  if (request.type !== 'soul.get_context') {
    return true;
  }
  if (typeof sender?.url !== 'string' || sender.url.startsWith('chrome-extension://')) {
    return false;
  }
  let urlOrigin: string;
  try {
    urlOrigin = new URL(sender.url).origin;
  } catch {
    return false;
  }
  if (urlOrigin !== request.origin) {
    return false;
  }
  return sender.origin === undefined || sender.origin === request.origin;
}

/** Отклоняет запрос по правилам host-а (fail-closed до отправки). */
export function validateOutgoingRequest(message: unknown): ValidationResult {
  if (typeof message !== 'object' || message === null) {
    return fail('invalid_request', 'Запрос не является объектом JSON.');
  }
  const m = message as Record<string, unknown>;
  if (m.protocol !== PROTOCOL_VERSION) {
    return fail('invalid_protocol', `Неизвестная версия протокола: "${String(m.protocol)}".`);
  }
  if (m.extensionId !== EXTENSION_ID) {
    return fail(
      'invalid_extension_id',
      `Неизвестный идентификатор расширения: "${String(m.extensionId)}".`,
    );
  }
  const nonce = m.nonce;
  if (typeof nonce !== 'string' || !isValidNonce(nonce)) {
    return fail('invalid_nonce', 'Nonce должен быть строкой 16–64 символов [A-Za-z0-9_-].');
  }

  if (m.type === 'soul.ping') {
    return {
      ok: true,
      request: { type: 'soul.ping', protocol: PROTOCOL_VERSION, extensionId: EXTENSION_ID, nonce },
    };
  }
  if (m.type === 'soul.get_context') {
    const origin = m.origin;
    if (typeof origin !== 'string' || !SUPPORTED_ORIGINS.includes(origin)) {
      return fail(
        'invalid_origin',
        `Происхождение "${String(origin)}" не входит в список поддерживаемых сайтов.`,
      );
    }
    const task = m.task;
    if (typeof task !== 'string' || characterCount(task) > MAX_TASK_CHARS) {
      return fail('task_too_long', `Текст задачи превышает ${MAX_TASK_CHARS} символов.`);
    }
    const maxTokens = m.maxTokens;
    if (
      typeof maxTokens !== 'number' ||
      !Number.isInteger(maxTokens) ||
      maxTokens < 1 ||
      maxTokens > MAX_TOKENS
    ) {
      return fail('invalid_request', `maxTokens должен быть целым числом от 1 до ${MAX_TOKENS}.`);
    }
    const request: GetContextRequest = {
      type: 'soul.get_context',
      protocol: PROTOCOL_VERSION,
      extensionId: EXTENSION_ID,
      nonce,
      origin,
      task,
      maxTokens,
    };
    if (serializedByteLength(m) > MAX_FRAME_BYTES) {
      return fail('request_too_large', 'Запрос превышает максимальный размер сообщения.');
    }
    return { ok: true, request };
  }
  return fail('unsupported_request', `Неизвестный тип запроса: "${String(m.type)}".`);
}

function fail(code: string, message: string): ValidationResult {
  return { ok: false, error: errorResponse(code, message) };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function isIntegerInRange(value: unknown, min: number, max: number): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= min && value <= max;
}

function hasOnlyKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const keys = Object.keys(value);
  return keys.length === expected.length && keys.every((key) => expected.includes(key));
}

function characterCount(value: string): number {
  return Array.from(value).length;
}

function utf8ByteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function serializedByteLength(value: unknown): number {
  try {
    return utf8ByteLength(JSON.stringify(value));
  } catch {
    return Number.POSITIVE_INFINITY;
  }
}
