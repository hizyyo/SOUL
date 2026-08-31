/**
 * Типизированная модель детерминированных политик SOUL.
 *
 * Зеркало src-tauri/src/policy.rs: только безопасные типизированные условия,
 * эффекты allow/deny/require_confirmation/redact, никакого eval. Все функции
 * чистые и детерминированные; авторитетный парсер и валидатор — Rust-бэкенд,
 * здесь — только UX-пресеты и лёгкая пред-проверка формы.
 */

export type Effect = 'allow' | 'deny' | 'require_confirmation' | 'redact';

export const EFFECTS: readonly Effect[] = ['allow', 'deny', 'require_confirmation', 'redact'];

/** Решётка §12.3: deny > require_confirmation > redact > allow. */
export const EFFECT_RANK: Record<Effect, number> = {
  deny: 4,
  require_confirmation: 3,
  redact: 2,
  allow: 1,
};

export function effectLabel(effect: Effect): string {
  switch (effect) {
    case 'allow':
      return 'Разрешить';
    case 'deny':
      return 'Запретить';
    case 'require_confirmation':
      return 'Требовать подтверждение';
    case 'redact':
      return 'Скрыть данные';
  }
}

/** Строка таблицы `policies`, возвращаемая Rust-командами. */
export interface PolicyRow {
  id: string;
  priority: number;
  enabled: boolean;
  rule_json: string;
  created_at: string;
  updated_at: string;
}

/** Результат оценки действия политиками (policy::Decision). */
export interface Decision {
  effect: Effect;
  rule_id: string | null;
  message: string | null;
}

/** Структурированное действие (§12.8) — как принимает Rust-бэкенд. */
export interface SoulAction {
  actionId: string;
  kind: string;
  actor: string;
  connectorId: string;
  accountId: string;
  environment: string;
  recipient?: string;
  domain?: string;
  amount?: number;
  currency?: string;
  dataClasses?: string[];
  reversible?: boolean;
  confirmedByUser?: boolean;
  requestedScopes?: string[];
  payloadHash?: string;
}

/** Лимиты, зеркалящие константы policy.rs (для подсказок в UI). */
export const MAX_PRIORITY = 10_000;
export const MAX_RULE_JSON_CHARS = 4_096;
export const MAX_RULE_MESSAGE_CHARS = 500;
export const MAX_RULE_ID_CHARS = 128;

export interface PolicyPreset {
  id: string;
  label: string;
  description: string;
  /** Детерминированный строитель JSON правила. */
  build: () => string;
}

const ruleJson = (parts: {
  id: string;
  priority: number;
  when: string;
  effect: Effect;
  message: string;
}): string =>
  JSON.stringify(
    {
      id: parts.id,
      priority: parts.priority,
      when: JSON.parse(parts.when),
      effect: parts.effect,
      message: parts.message,
    },
    null,
    2,
  );

/**
 * Пресеты — зеркало дефолтов Rust-сида плюс демо-примеры для UI.
 * id пресета совпадает с id правила (как у сида), повторное создание
 * отклонит бэкенд с UNIQUE-ошибкой — это нормальный UX.
 */
export const POLICY_PRESETS: readonly PolicyPreset[] = [
  {
    id: 'policy_high_value_confirmation',
    label: 'Крупная покупка → подтверждение',
    description: 'Покупки дороже $500 требуют подтверждения пользователя (сид).',
    build: () =>
      ruleJson({
        id: 'policy_high_value_confirmation',
        priority: 900,
        when: `{"all":[{"eq":["action.kind","purchase.create"]},{"gt":["action.amount",500]}]}`,
        effect: 'require_confirmation',
        message: 'Purchases above $500 require confirmation.',
      }),
  },
  {
    id: 'policy_destructive_denied',
    label: 'Необратимое → запрет',
    description: 'Необратимые действия запрещены, пока пользователь не подтвердил их явно (сид).',
    build: () =>
      ruleJson({
        id: 'policy_destructive_denied',
        priority: 1000,
        when: `{"all":[{"eq":["action.reversible",false]},{"eq":["action.confirmedByUser",false]}]}`,
        effect: 'deny',
        message: 'Irreversible actions are denied unless explicitly confirmed by the user.',
      }),
  },
  {
    id: 'policy_recipient_domain',
    label: 'Получатель в домене',
    description: 'Действия для получателя из подозрительного домена отклоняются.',
    build: () =>
      ruleJson({
        id: 'policy_recipient_domain',
        priority: 800,
        when: `{"in":["action.domain",["untrusted-io","suspicious.dev"]]}`,
        effect: 'deny',
        message: 'Actions targeting untrusted domains are denied.',
      }),
  },
  {
    id: 'policy_production_read_only',
    label: 'Окружение → только чтение',
    description: 'Запись в production запрещена; демо-приложение может читать.',
    build: () =>
      ruleJson({
        id: 'policy_production_read_only',
        priority: 700,
        when: `{"any":[{"eq":["action.environment","production"]},{"eq":["action.environment","staging"]}]}`,
        effect: 'redact',
        message: 'Production and staging data is redacted outside the sandbox.',
      }),
  },
];

export function presetById(id: string): PolicyPreset | undefined {
  return POLICY_PRESETS.find((p) => p.id === id);
}

/** Пример действия для playground'а оценки. */
export const EVALUATION_EXAMPLE: string = JSON.stringify(
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
  } satisfies SoulAction,
  null,
  2,
);

export interface RuleFormState {
  ok: boolean;
  error: string | null;
  priority: number;
  effect: Effect | null;
}

/**
 * Лёгкая пред-проверка JSON правила перед отправкой в Rust (авторитет —
 * бэкенд). Проверяет: парсится ли JSON, тип полей, диапазон priority,
 * известность effect, непустой id.
 */
export function validateRuleJson(raw: string): RuleFormState {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { ok: false, error: 'Rule is not valid JSON.', priority: 0, effect: null };
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return {
      ok: false,
      error: 'Rule must be a JSON object (SoulRule).',
      priority: 0,
      effect: null,
    };
  }
  const rule = parsed as Record<string, unknown>;
  if (typeof rule.id !== 'string' || rule.id.trim().length === 0) {
    return { ok: false, error: 'Rule id must not be empty.', priority: 0, effect: null };
  }
  if (rule.id.length > MAX_RULE_ID_CHARS) {
    return {
      ok: false,
      error: `Rule id exceeds ${MAX_RULE_ID_CHARS} characters.`,
      priority: 0,
      effect: null,
    };
  }
  if (typeof rule.priority !== 'number' || !Number.isInteger(rule.priority)) {
    return { ok: false, error: 'Rule priority must be an integer.', priority: 0, effect: null };
  }
  if (rule.priority < 0 || rule.priority > MAX_PRIORITY) {
    return {
      ok: false,
      error: `Rule priority must be between 0 and ${MAX_PRIORITY}.`,
      priority: rule.priority,
      effect: null,
    };
  }
  if (typeof rule.effect !== 'string' || !EFFECTS.includes(rule.effect as Effect)) {
    return {
      ok: false,
      error: `Effect must be one of: ${EFFECTS.join(', ')}.`,
      priority: rule.priority,
      effect: null,
    };
  }
  if (raw.length > MAX_RULE_JSON_CHARS) {
    return {
      ok: false,
      error: `Rule exceeds ${MAX_RULE_JSON_CHARS} characters.`,
      priority: rule.priority,
      effect: rule.effect as Effect,
    };
  }
  if (typeof rule.message === 'string' && rule.message.length > MAX_RULE_MESSAGE_CHARS) {
    return {
      ok: false,
      error: `Rule message exceeds ${MAX_RULE_MESSAGE_CHARS} characters.`,
      priority: rule.priority,
      effect: rule.effect as Effect,
    };
  }
  return { ok: true, error: null, priority: rule.priority, effect: rule.effect as Effect };
}

/** Эффект правила из его JSON (для бейджа в списке); null, если JSON битый. */
export function effectOfRuleJson(raw: string): Effect | null {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) return null;
    const effect = (parsed as Record<string, unknown>).effect;
    return typeof effect === 'string' && EFFECTS.includes(effect as Effect)
      ? (effect as Effect)
      : null;
  } catch {
    return null;
  }
}
