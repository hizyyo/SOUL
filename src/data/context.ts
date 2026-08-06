import { domainForQuestion, parseEntityData, type SensitivityLevel } from './review';

export type { SensitivityLevel };

/**
 * Детерминированный компилятор контекста SESSION-07.
 *
 * Чистые синхронные функции: без сети, без модели, без часов и случайности —
 * одинаковое состояние SOUL + одинаковый запрос всегда дают одинаковый пак.
 * Отвечает за минимальный разрешённый контекст задачи: фильтры области,
 * чувствительности, состояния и времени; границы выше предпочтений и фактов;
 * удаление замещённых дубликатов; явные конфликты; упаковку токенов со
 * стандартом 900 и жёстким максимумом 3 000.
 */

/** Версия правил компиляции — меняется только при изменении самих правил. */
export const CONTEXT_POLICY_VERSION = 'soul-context-policy/1';

/** Стандартный бюджет упаковки токенов. */
export const CONTEXT_STANDARD_TOKENS = 900;

/** Жёсткий максимум: пак никогда не превышает этот бюджет. */
export const CONTEXT_HARD_MAX_TOKENS = 3_000;

/** Статусы, разрешённые в контексте по умолчанию: только разрешённые (active). */
export const DEFAULT_ALLOWED_STATUSES: readonly string[] = ['active'];

/** Disclosure default: sensitive/restricted require explicit selection. */
export const DEFAULT_ALLOWED_SENSITIVITY: readonly SensitivityLevel[] = [
  'public',
  'internal',
  'private',
];

const ALL_SENSITIVITY: readonly SensitivityLevel[] = [
  'public',
  'internal',
  'private',
  'sensitive',
  'restricted',
];

/** Приоритет типов: границы всегда выше предпочтений и фактов. */
const PRIORITY_TIER: Record<string, number> = {
  boundary: 4,
  decision: 3,
  goal: 2,
  preference: 1,
  fact: 1,
};

/** Полная форма сущности, возвращаемая Rust-бэкендом (list/search commands). */
export interface ContextEntity {
  id: string;
  soul_id: string;
  entity_type: string;
  status: string;
  data: string;
  created_at: string;
  updated_at: string;
}

export interface ContextQuery {
  text: string;
  /** Разрешённые области; пустой массив = без ограничения по этому измерению. */
  domains: string[];
  projects: string[];
  people: string[];
  channels: string[];
  /** Разрешённые уровни чувствительности; пустой массив = public/internal/private. */
  sensitivity: SensitivityLevel[];
  /** Разрешённые статусы; пустой массив = только active. */
  statuses: string[];
  /** ISO-строки; сущности вне окна [since, until] исключаются. */
  since?: string | null;
  until?: string | null;
  /** Бюджет токенов: от 1 до 3000, по умолчанию 900. */
  maxTokens: number;
}

export interface ContextItem {
  id: string;
  entityType: string;
  status: string;
  claim: string;
  evidence: string;
  sensitivity: SensitivityLevel;
  domains: string[];
  relevance: number;
  priority: number;
  confidence: number;
  updatedAt: string;
}

export interface ContextConflict {
  a: string;
  b: string;
  reason: string;
}

export interface ContextPack {
  items: ContextItem[];
  conflicts: ContextConflict[];
  /** ID сущностей, удалённых как замещённые дубликаты (нет в паке). */
  supersededIds: string[];
  policyVersion: string;
  /** Детерминированная версия состояния: хэш включённых сущностей. */
  stateVersion: string;
  maxTokens: number;
  tokenEstimate: number;
  serialized: string;
}

/** Оценочная стоимость входных токенов, USD (SESSION-14). */
export const COST_USD_PER_1K_INPUT_TOKENS = 0.005;

export function costEstimateUsd(tokenEstimate: number): number {
  return (tokenEstimate / 1000) * COST_USD_PER_1K_INPUT_TOKENS;
}

function isCjk(code: number): boolean {
  return (
    (code >= 0x4e00 && code <= 0x9fff) || // CJK Unified
    (code >= 0x3400 && code <= 0x4dbf) || // CJK Ext A
    (code >= 0x20000 && code <= 0x2fa1f) || // CJK Ext B+
    (code >= 0x3040 && code <= 0x30ff) || // Hiragana + Katakana
    (code >= 0xac00 && code <= 0xd7af) || // Hangul
    (code >= 0xff00 && code <= 0xffef) || // Fullwidth
    (code >= 0x3000 && code <= 0x303f) // CJK punctuation
  );
}

/**
 * Консервативная детерминированная оценка токенов без модели.
 * CJK-символ ~1 токен, остальные символы ~1/3 токена (безопасный верхний
 * предел против реальных токенизаторов). Оценка применяется к реальному
 * сериализованному пакету, поэтому превысить жёсткий максимум невозможно.
 */
export function estimateTokens(text: string): number {
  if (!text) return 0;
  let units = 0;
  for (const ch of text) {
    const code = ch.codePointAt(0);
    units += code !== undefined && isCjk(code) ? 1 : 1 / 3;
  }
  return Math.ceil(units);
}

/** 32-битный FNV-1a: детерминированный, без потери точности (imul). */
function hashString(text: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, '0');
}

export function defaultQuery(): ContextQuery {
  return {
    text: '',
    domains: [],
    projects: [],
    people: [],
    channels: [],
    sensitivity: [...DEFAULT_ALLOWED_SENSITIVITY],
    statuses: [...DEFAULT_ALLOWED_STATUSES],
    since: null,
    until: null,
    maxTokens: CONTEXT_STANDARD_TOKENS,
  };
}

function dataOf(entity: ContextEntity): ReturnType<typeof parseEntityData> {
  return parseEntityData(entity.data);
}

function sensitivityOf(entity: ContextEntity): SensitivityLevel {
  const data = dataOf(entity);
  const value = data.sensitivity;
  return ALL_SENSITIVITY.includes(value as SensitivityLevel)
    ? (value as SensitivityLevel)
    : 'internal';
}

function domainsOf(entity: ContextEntity): string[] {
  const data = dataOf(entity);
  if (Array.isArray(data.scope?.domains) && data.scope.domains.length > 0) {
    return data.scope.domains.filter((d): d is string => typeof d === 'string');
  }
  const questionId = data.questionId;
  if (typeof questionId === 'string' && questionId) {
    return [domainForQuestion(questionId)];
  }
  return [];
}

function questionIdOf(entity: ContextEntity): string | null {
  const q = dataOf(entity).questionId;
  return typeof q === 'string' && q.length > 0 ? q : null;
}

function valueOf(entity: ContextEntity): unknown {
  return dataOf(entity).value;
}

function confidenceOf(entity: ContextEntity): number {
  const c = dataOf(entity).confidence;
  return typeof c === 'number' && Number.isFinite(c) ? Math.min(1, Math.max(0, c)) : 0;
}

function scopeDim(entity: ContextEntity, dim: 'projects' | 'people' | 'channels'): string[] {
  const values = dataOf(entity).scope?.[dim];
  if (!Array.isArray(values)) return [];
  return values.filter((v): v is string => typeof v === 'string');
}

/** Токенизация для релевантности: строчные unicode-слова. */
function tokenize(text: string): Set<string> {
  return new Set(
    text
      .toLowerCase()
      .split(/[^\p{L}\p{N}]+/u)
      .filter((w) => w.length > 0),
  );
}

/**
 * Релевантность сущности запросу: термин в claim ×2, в evidence ×1.
 * Пустой запрос — релевантность 0 для всех (пак собирается без текста).
 */
export function relevanceOf(entity: ContextEntity, queryText: string): number {
  if (!queryText.trim()) return 0;
  const terms = tokenize(queryText);
  if (terms.size === 0) return 0;
  const claim = tokenize(dataOf(entity).claim ?? entity.data);
  const evidence = tokenize(dataOf(entity).evidence ?? '');
  let score = 0;
  for (const term of terms) {
    if (claim.has(term)) score += 2;
    if (evidence.has(term)) score += 1;
  }
  return score;
}

/** Ограничение по одному измерению области: пусто = без ограничения. */
function matchesScopeDimension(entityValues: string[], allowed: string[]): boolean {
  if (allowed.length === 0) return true;
  return allowed.some((a) => entityValues.includes(a));
}

/** Проверка вхождения сущности в окно времени (RFC3339/ISO). */
function inTimeWindow(createdAt: string, since?: string | null, until?: string | null): boolean {
  if (!since && !until) return true;
  const ts = Date.parse(createdAt);
  if (Number.isNaN(ts)) return true;
  if (since) {
    const s = Date.parse(since);
    if (!Number.isNaN(s) && ts < s) return false;
  }
  if (until) {
    const u = Date.parse(until);
    if (!Number.isNaN(u) && ts > u) return false;
  }
  return true;
}

export interface DedupResult {
  kept: ContextEntity[];
  supersededIds: string[];
}

/**
 * Удаление замещённых дубликатов: из сущностей с одинаковым questionId
 * (перекомпиляция калибровки) остаётся только самая свежая по updated_at.
 * Сущности без questionId (legacy) не трогаются. Старые ответы помечаются
 * в supersededIds — они не попадают в пак, но видны в отчёте.
 */
export function dedupeSuperseded(entities: ContextEntity[]): DedupResult {
  const byQuestion = new Map<string, ContextEntity[]>();
  const standalone: ContextEntity[] = [];
  for (const entity of entities) {
    const q = questionIdOf(entity);
    if (q === null) {
      standalone.push(entity);
    } else {
      const list = byQuestion.get(q);
      if (list) list.push(entity);
      else byQuestion.set(q, [entity]);
    }
  }
  const kept: ContextEntity[] = [...standalone];
  const supersededIds: string[] = [];
  for (const [, group] of byQuestion) {
    const sorted = [...group].sort((a, b) => {
      const recency = b.updated_at.localeCompare(a.updated_at);
      if (recency !== 0) return recency;
      return a.id.localeCompare(b.id);
    });
    const newest = sorted[0];
    if (!newest) continue;
    kept.push(newest);
    for (const old of sorted.slice(1)) supersededIds.push(old.id);
  }
  return { kept, supersededIds };
}

/**
 * Явные конфликты: разные ответы на один калибровочный вопрос. Считается по
 * ВСЕМ входным сущностям (до дедупликации), поэтому повторный ответ с другим
 * значением даёт пару (старое значение → новое). Пары детерминированы
 * (сортировка по id), каждая сущность участвует максимум в одной паре.
 */
export function detectConflicts(entities: ContextEntity[]): ContextConflict[] {
  const byQuestion = new Map<string, ContextEntity[]>();
  for (const entity of entities) {
    const q = questionIdOf(entity);
    if (q === null) continue;
    const list = byQuestion.get(q);
    if (list) list.push(entity);
    else byQuestion.set(q, [entity]);
  }
  const conflicts: ContextConflict[] = [];
  for (const [questionId, group] of byQuestion) {
    const byValue = new Map<string, ContextEntity[]>();
    for (const entity of group) {
      const key = JSON.stringify(valueOf(entity));
      const list = byValue.get(key);
      if (list) list.push(entity);
      else byValue.set(key, [entity]);
    }
    if (byValue.size < 2) continue;
    const representatives: ContextEntity[] = [];
    for (const group of byValue.values()) {
      const first = group[0];
      if (first) representatives.push(first);
    }
    representatives.sort((a, b) => a.id.localeCompare(b.id));
    const anchor = representatives[0];
    if (!anchor) continue;
    for (let i = 1; i < representatives.length; i++) {
      const other = representatives[i];
      if (!other) continue;
      conflicts.push({
        a: anchor.id,
        b: other.id,
        reason: `Same calibration question (${questionId}) with different answers`,
      });
    }
  }
  return conflicts;
}

function priorityOf(entity: ContextEntity): number {
  return PRIORITY_TIER[entity.entity_type] ?? 0;
}

/** Полный детерминированный порядок: приоритет → релевантность → уверенность → свежесть → id. */
function compareItems(a: ContextItem, b: ContextItem): number {
  if (b.priority !== a.priority) return b.priority - a.priority;
  if (b.relevance !== a.relevance) return b.relevance - a.relevance;
  if (b.confidence !== a.confidence) return b.confidence - a.confidence;
  const recency = b.updatedAt.localeCompare(a.updatedAt);
  if (recency !== 0) return recency;
  return a.id.localeCompare(b.id);
}

function toContextItem(entity: ContextEntity, queryText: string): ContextItem {
  const data = dataOf(entity);
  return {
    id: entity.id,
    entityType: entity.entity_type,
    status: entity.status,
    claim: typeof data.claim === 'string' ? data.claim : '',
    evidence: typeof data.evidence === 'string' ? data.evidence : '',
    sensitivity: sensitivityOf(entity),
    domains: domainsOf(entity),
    relevance: relevanceOf(entity, queryText),
    priority: priorityOf(entity),
    confidence: confidenceOf(entity),
    updatedAt: entity.updated_at,
  };
}

function formatTokens(value: number): string {
  return value.toLocaleString('en-US');
}

/**
 * Главная детерминированная функция: компилирует минимальный разрешённый
 * контекст задачи. Текстовый запрос отсекает нерелевантные сущности полностью
 * (relevance == 0 не попадает в пак); бюджет никогда не превышается.
 */
export function compileContext(entities: ContextEntity[], query: ContextQuery): ContextPack {
  const raw = Math.floor(query.maxTokens);
  const maxTokens = Number.isFinite(raw)
    ? Math.max(1, Math.min(CONTEXT_HARD_MAX_TOKENS, raw))
    : CONTEXT_STANDARD_TOKENS;
  const allowedSensitivity =
    query.sensitivity.length > 0 ? query.sensitivity : [...DEFAULT_ALLOWED_SENSITIVITY];
  const allowedStatuses =
    query.statuses.length > 0 ? query.statuses : [...DEFAULT_ALLOWED_STATUSES];
  const queryText = query.text.trim();

  // Конфликты считаются по сырым данным: повторные ответы на один вопрос с
  // другим значением дают пару (старое значение → новое) ещё до дедупликации.
  const conflicts = detectConflicts(entities).sort((x, y) =>
    `${x.a}|${x.b}`.localeCompare(`${y.a}|${y.b}`),
  );
  const { kept, supersededIds } = dedupeSuperseded(entities);
  supersededIds.sort((a, b) => a.localeCompare(b));

  const eligible = kept.filter((entity) => {
    if (!allowedStatuses.includes(entity.status)) return false;
    if (!allowedSensitivity.includes(sensitivityOf(entity))) return false;
    if (!matchesScopeDimension(scopeDim(entity, 'projects'), query.projects)) return false;
    if (!matchesScopeDimension(scopeDim(entity, 'people'), query.people)) return false;
    if (!matchesScopeDimension(scopeDim(entity, 'channels'), query.channels)) return false;
    const entityDomains = domainsOf(entity);
    if (query.domains.length > 0 && !query.domains.some((d) => entityDomains.includes(d)))
      return false;
    if (!inTimeWindow(entity.created_at, query.since, query.until)) return false;
    if (queryText && relevanceOf(entity, queryText) <= 0) return false;
    return true;
  });

  const items = eligible.map((e) => toContextItem(e, queryText)).sort(compareItems);

  // Упаковка: добавляем в порядке приоритета, пока оценка ПОЛНОГО пакета
  // (заголовок + тело + отчёт о конфликтах/superseded) не превысит бюджет.
  // stateVersion — 8 hex-символов, размер не зависит от значения, поэтому
  // в пробной оценке используется заглушка.
  const packed: ContextItem[] = [];
  const candidateTexts: string[] = [];
  for (const item of items) {
    const lines: string[] = [
      `[${item.id}] ${item.entityType} / ${item.status} / ${item.sensitivity}`,
    ];
    lines.push(item.claim);
    if (item.evidence && item.evidence !== item.claim) lines.push(`evidence: ${item.evidence}`);
    const text = lines.join('\n');
    const trialBody = [...candidateTexts, text].join('\n');
    const trial = serializePackBodyParts(
      trialBody,
      conflicts,
      supersededIds,
      '00000000',
      candidateTexts.length + 1,
      maxTokens,
    );
    if (estimateTokens(trial) <= maxTokens) {
      packed.push(item);
      candidateTexts.push(text);
    }
  }

  // Финальная сборка: версия состояния по включённым сущностям; токены в
  // заголовке — оценка реального сериализованного текста (не заглушки).
  const finalize = (count: number) => {
    const selected = packed.slice(0, count);
    const body = candidateTexts.slice(0, count).join('\n');
    const stateSource = selected
      .map((i) => `${i.id}|${i.updatedAt}`)
      .sort((a, b) => a.localeCompare(b))
      .join('\n');
    const stateVersion = hashString(stateSource);
    const draft = serializePackBodyParts(
      body,
      conflicts,
      supersededIds,
      stateVersion,
      selected.length,
      maxTokens,
    );
    const tokenEstimate = estimateTokens(draft);
    const serialized = draft.replace('tokens: X of', `tokens: ${formatTokens(tokenEstimate)} of`);
    return { items: selected, stateVersion, tokenEstimate: estimateTokens(serialized), serialized };
  };

  // Страховка: замена 'X' на число добавляет до пары символов — если оценка
  // финального текста всё же превысила бюджет, сбрасываем самый низкоприоритетный
  // элемент и пересобираем. Детерминированно, максимум пара итераций.
  let final = finalize(packed.length);
  while (final.tokenEstimate > maxTokens && final.items.length > 0) {
    final = finalize(final.items.length - 1);
  }

  return {
    ...final,
    conflicts,
    supersededIds,
    policyVersion: CONTEXT_POLICY_VERSION,
    maxTokens,
  };
}

function serializePackBodyParts(
  body: string,
  conflicts: ContextConflict[],
  supersededIds: string[],
  stateVersion: string,
  itemCount: number,
  maxTokens: number,
): string {
  const parts = [
    'SOUL CONTEXT',
    `policy: ${CONTEXT_POLICY_VERSION}`,
    `state: ${stateVersion}`,
    `tokens: X of ${formatTokens(maxTokens)}`,
    `entities: ${itemCount}`,
    body,
  ];
  if (conflicts.length > 0) {
    parts.push('CONFLICTS:');
    for (const c of conflicts) parts.push(`- ${c.a} vs ${c.b}: ${c.reason}`);
  }
  if (supersededIds.length > 0) parts.push(`SUPERSEDED: ${supersededIds.join(', ')}`);
  return parts.join('\n');
}

/** Все области, встречающиеся в сущностях (для фильтра в UI). */
export function collectDomains(entities: ContextEntity[]): string[] {
  const set = new Set<string>();
  for (const entity of entities) {
    for (const d of domainsOf(entity)) set.add(d);
  }
  return [...set].sort((a, b) => a.localeCompare(b));
}

/** Разбивка сущностей по статусам (для UI). */
export function entityCounts(entities: ContextEntity[]): { status: string; count: number }[] {
  const byStatus = new Map<string, number>();
  for (const entity of entities) {
    byStatus.set(entity.status, (byStatus.get(entity.status) ?? 0) + 1);
  }
  return [...byStatus.entries()]
    .map(([status, count]) => ({ status, count }))
    .sort((a, b) => a.status.localeCompare(b.status));
}
