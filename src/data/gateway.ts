/**
 * Модель имитированного Gateway SESSION-12 (ULTRA_MVP §4.11).
 *
 * Зеркало src-tauri/src/gateway.rs: локальная имитация внешнего действия через
 * поддельный коннектор — никаких реальных внешних вызовов, никакого управления
 * произвольными внешними агентами. Rust — авторитетный источник правды; здесь
 * только типы, константы, лёгкая пред-проверка формы и UX-лейблы.
 *
 * Review-pass: capability привязывает канал (коннектор/учётка/окружение),
 * подписана локальным устройством (ed25519); capability и квитанции несут
 * подпись и флаг `signature_valid` (честный аудит подделок); статус `held`
 * имеет продолжение — подтверждение пользователем; статус `redacted` имеет
 * продолжение — поддельный коннектор получает отредактированную копию;
 * реестр имитированных коннекторов управляется из интерфейса.
 */

import type { Decision, Effect } from './policy';

/** Статус квитанции имитации (gateway::GatewayStatus). */
export type GatewayStatus = 'pending' | 'simulated' | 'denied' | 'held' | 'redacted' | 'refused';

export const GATEWAY_STATUSES: readonly GatewayStatus[] = [
  'pending',
  'simulated',
  'denied',
  'held',
  'redacted',
  'refused',
];

/** Точная метка имитации из §4.11. */
export const SIMULATION_LABEL = 'Имитация: внешнее действие не выполнялось.';

/** Срок capability по умолчанию и верхняя граница (зеркало gateway.rs). */
export const GATEWAY_DEFAULT_TTL = 300;
export const GATEWAY_MAX_TTL = 3_600;
export const MAX_ACTION_JSON_CHARS = 16_000;
/** Лимиты реестра имитированных коннекторов (зеркало gateway.rs). */
export const GATEWAY_MAX_CONNECTORS = 50;
export const GATEWAY_MAX_CHANNEL_FIELD_CHARS = 64;

/** Эффект решения при выдаче capability (зеркало gateway.rs). */
export type GatewayDecisionEffect = 'allow' | 'require_confirmation' | 'redact';

/** Локальная имитированная capability (gateway::CapabilityInfo). */
export interface GatewayCapability {
  id: string;
  action_id: string;
  kind: string;
  payload_hash: string;
  nonce: string;
  expires_at: string;
  created_at: string;
  used_at: string | null;
  /** Канал, к которому capability привязана при выдаче. */
  connector_id: string;
  account_id: string;
  environment: string;
  /** Эффект решения при выдаче: allow / require_confirmation / redact. */
  decision_effect: GatewayDecisionEffect;
  /** Подтверждена ли capability пользователем (для require_confirmation). */
  confirmed_by_user: boolean;
  /** Действие выполнялось бы с отредактированной нагрузкой (redact). */
  redacted: boolean;
  /** Подпись локального устройства (base64, ed25519). */
  signature: string;
  signer_public_key: string;
  /** Подпись проверена по сохранённому публичному ключу. */
  signature_valid: boolean;
}

/** Квитанция gateway (gateway::GatewayReceipt) — без исходной нагрузки. */
export interface GatewayReceipt {
  id: string;
  capability_id: string | null;
  action_id: string;
  kind: string;
  status: GatewayStatus;
  decision_effect: Effect;
  rule_id: string | null;
  message: string | null;
  connector_executed: boolean;
  reason: string | null;
  nonce: string | null;
  created_at: string;
  /** Подпись локального устройства (base64, ed25519). */
  signature: string;
  signer_public_key: string;
  /** Подпись проверена по сохранённому публичному ключу. */
  signature_valid: boolean;
}

/** Результат этапа предложения (gateway::GatewayProposal). */
export interface GatewayProposal {
  decision: Decision;
  capability: GatewayCapability | null;
  receipt: GatewayReceipt;
}

/** Результат выполнения capability (gateway::GatewayExecuteResult). */
export interface GatewayExecuteResult {
  ok: boolean;
  receipt: GatewayReceipt;
}

export function gatewayStatusLabel(status: GatewayStatus): string {
  switch (status) {
    case 'pending':
      return 'Capability выдана';
    case 'simulated':
      return 'Имитация выполнена';
    case 'denied':
      return 'Запрещено политикой';
    case 'held':
      return 'Требует подтверждения';
    case 'redacted':
      return 'Скрыто политикой';
    case 'refused':
      return 'Отказ';
  }
}

/** Тон статуса для бейджей. */
export const GATEWAY_STATUS_TONE: Record<GatewayStatus, { bg: string; fg: string }> = {
  pending: { bg: '#fffbeb', fg: '#b45309' },
  simulated: { bg: '#ecfdf5', fg: '#047857' },
  denied: { bg: '#fef2f2', fg: '#dc2626' },
  held: { bg: '#fff7ed', fg: '#c2410c' },
  redacted: { bg: '#eff6ff', fg: '#1d4ed8' },
  refused: { bg: '#f3f4f6', fg: '#6b7280' },
};

/** Канал выполнения (коннектор, учётная запись, окружение). */
export interface GatewayChannel {
  connector_id: string;
  account_id: string;
  environment: string;
}

/**
 * Сид реестра имитированных коннекторов — зеркало seed gateway.rs. Живой
 * реестр приходит из `list_gateway_connectors_cmd`; этот список — только
 * начальные варианты (и фолбэк, пока реестр не загружен).
 */
export const GATEWAY_CONNECTOR_OPTIONS: readonly GatewayChannel[] = [
  { connector_id: 'demo-connector', account_id: 'acct-1', environment: 'production' },
  { connector_id: 'demo-connector', account_id: 'acct-1', environment: 'staging' },
  { connector_id: 'demo-connector', account_id: 'acct-2', environment: 'production' },
  { connector_id: 'sandbox-connector', account_id: 'acct-1', environment: 'development' },
];

export function channelLabel(channel: GatewayChannel): string {
  return `${channel.connector_id} · ${channel.account_id} · ${channel.environment}`;
}

/** Пред-проверка полей канала перед отправкой в Rust (авторитет — бэкенд). */
export function validateChannelInput(
  connectorId: string,
  accountId: string,
  environment: string,
): { ok: boolean; error: string | null } {
  for (const [name, value] of [
    ['connectorId', connectorId],
    ['accountId', accountId],
    ['environment', environment],
  ] as const) {
    if (value.trim().length === 0) {
      return { ok: false, error: `Channel field '${name}' must not be empty.` };
    }
    if (value.trim().length > GATEWAY_MAX_CHANNEL_FIELD_CHARS) {
      return {
        ok: false,
        error: `Channel field '${name}' exceeds ${GATEWAY_MAX_CHANNEL_FIELD_CHARS} characters.`,
      };
    }
  }
  return { ok: true, error: null };
}

/** Пример действия для Gateway-демо §4.11: покупка на $600 в production. */
export const GATEWAY_EXAMPLE_ACTION: string = JSON.stringify(
  {
    actionId: 'act_0001',
    kind: 'purchase.create',
    actor: 'agent-1',
    connectorId: 'demo-connector',
    accountId: 'acct-1',
    environment: 'production',
    recipient: 'acme-vendor',
    domain: 'acme.com',
    amount: 600,
    currency: 'USD',
    dataClasses: ['purchase'],
    reversible: false,
    confirmedByUser: false,
    payloadHash: 'agent-proposed-hash',
  },
  null,
  2,
);

/** Легкая пред-проверка JSON действия перед отправкой в Rust (авторитет — бэкенд). */
export function validateActionJson(raw: string): { ok: boolean; error: string | null } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: 'Action is not valid JSON.' };
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return { ok: false, error: 'Action must be a JSON object (SoulAction).' };
  }
  if (raw.length > MAX_ACTION_JSON_CHARS) {
    return { ok: false, error: `Action exceeds ${MAX_ACTION_JSON_CHARS} characters.` };
  }
  const action = parsed as Record<string, unknown>;
  for (const field of ['actionId', 'kind', 'actor', 'connectorId', 'accountId']) {
    if (typeof action[field] !== 'string' || action[field].trim().length === 0) {
      return {
        ok: false,
        error: `Required action field '${field}' must not be empty.`,
      };
    }
  }
  return { ok: true, error: null };
}

/** Состояние capability для UI: удерживается / готова / использована / истекла. */
export function capabilityState(cap: GatewayCapability): 'held' | 'ready' | 'used' | 'expired' {
  if (cap.used_at !== null) return 'used';
  if (Date.parse(cap.expires_at) < Date.now()) return 'expired';
  if (cap.decision_effect === 'require_confirmation' && !cap.confirmed_by_user) {
    return 'held';
  }
  return 'ready';
}

/** Короткий отображаемый фрагмент хэша/nonce/подписи. */
export function shortDigest(value: string): string {
  return value.length > 12 ? `${value.slice(0, 12)}…` : value;
}
