/**
 * Протокол soul-bridge/1: типы запросов/ответов и валидация исходящих
 * запросов. Зеркалирует src-tauri/src/bridge.rs (коды ошибок и лимиты).
 */

import {
  EXTENSION_ID,
  MAX_FRAME_BYTES,
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
  extensionId: string;
  nonce: string;
}

export interface ContextResponse {
  type: 'soul.context';
  pack: string;
  entityCount: number;
  tokenEstimate: number;
  /** 8 hex-символов: версия policy-кодекса (host шлёт строкой, см. context.rs). */
  policyVersion: string;
  /** 8 hex-символов: хэш состояния БД (host шлёт строкой, см. context.rs). */
  stateVersion: string;
  maxTokens: number;
}

export interface ErrorResponse {
  type: 'soul.error';
  code: string;
  message: string;
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

export function isContextResponse(value: unknown): value is ContextResponse {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { type?: unknown }).type === 'soul.context' &&
    typeof (value as { pack?: unknown }).pack === 'string' &&
    typeof (value as { entityCount?: unknown }).entityCount === 'number' &&
    typeof (value as { policyVersion?: unknown }).policyVersion === 'string' &&
    typeof (value as { stateVersion?: unknown }).stateVersion === 'string'
  );
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
    if (typeof task !== 'string' || task.length > MAX_TASK_CHARS) {
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
    if (JSON.stringify(m).length > MAX_FRAME_BYTES) {
      return fail('request_too_large', 'Запрос превышает максимальный размер сообщения.');
    }
    return { ok: true, request };
  }
  return fail('unsupported_request', `Неизвестный тип запроса: "${String(m.type)}".`);
}

function fail(code: string, message: string): ValidationResult {
  return { ok: false, error: errorResponse(code, message) };
}
