import {
  compileContext,
  type ContextEntity,
  type ContextPack,
  CONTEXT_STANDARD_TOKENS,
} from './context';

/**
 * Blind Preference Test (SESSION-10) — чистая модель данных и статистики.
 *
 * Протокол (SOUL_MASTER_PLAN §6.2): один и тот же сценарий генерируется
 * пользователем в его AI-клиенте дважды — вариант с SOUL-контекстом и вариант
 * с коротким базовым профилем (B1); приложение перемешивает порядок (слот
 * назначает Rust), пользователь выбирает, какой ответ «больше похож на него»,
 * выбор фиксируется ДО раскрытия; статистика — win rate на не-тайных,
 * двусторонний точный биномиальный тест и Wilson 95% CI.
 *
 * Все функции детерминированы и не зависят от модели: 0 токенов, локально.
 */

/** Рекомендуемое число раундов для индивидуального демо (§6.7). */
export const EVAL_RECOMMENDED_ROUNDS = 20;

/** Ключ localStorage с редактируемым базовым профилем B1. */
export const B1_PROFILE_STORAGE_KEY = 'soul.b1.profile';

/** Доля выборки для share-карты — не публиковать без min(N). */
export const SHARE_MIN_ROUNDS = 20;

export interface BlindScenario {
  id: string;
  /** Тематическая область для группировки в UI. */
  domain: string;
  /** Вопрос-дилемма; текст идёт и в промпты, и как запрос компилятора. */
  question: string;
}

/**
 * Банк held-out дилемм: свежие сценарии, не из прошлых разговоров (§6.4).
 * Сценарии намеренно generic — они обязаны полагаться на контекст SOUL,
 * а не на упоминание конкретных сущностей.
 */
export const SCENARIO_BANK: readonly BlindScenario[] = [
  {
    id: 'scen_career_1',
    domain: 'career',
    question:
      'You receive two job offers on the same day: one from a stable company with a 20% higher salary and slow growth, another from a startup with lower pay but a faster learning curve and more ownership. Which one do you take, and why?',
  },
  {
    id: 'scen_career_2',
    domain: 'career',
    question:
      'Your manager asks you to lead a high-visibility project that you have never done before. You would need to learn on the job in front of the whole team. How do you respond?',
  },
  {
    id: 'scen_career_3',
    domain: 'career',
    question:
      'You have been in your current role for two years with no promotion in sight. A competitor offers you a similar role with more money but a worse commute and less flexibility. What do you do?',
  },
  {
    id: 'scen_work_1',
    domain: 'work',
    question:
      'A coworker regularly misses deadlines and asks you to cover for them. You have your own workload. How do you handle the next request?',
  },
  {
    id: 'scen_work_2',
    domain: 'work',
    question:
      'You spot a serious bug in code your teammate just shipped to production. The team is celebrating the release. Do you speak up now or later?',
  },
  {
    id: 'scen_work_3',
    domain: 'work',
    question:
      'Your team adopts a new tool that you believe is worse than the current one. The decision was already made. What do you do?',
  },
  {
    id: 'scen_money_1',
    domain: 'money',
    question:
      'You unexpectedly receive a large bonus. Your partner suggests spending half on a vacation, and saving the rest. You had planned to invest it. How do you decide?',
  },
  {
    id: 'scen_money_2',
    domain: 'money',
    question:
      'A friend asks you to lend them a significant amount of money for a business idea that sounds risky. How do you respond?',
  },
  {
    id: 'scen_money_3',
    domain: 'money',
    question:
      'Your bank offers you a low-interest loan to buy something you want but do not need. You can afford it. Do you take it?',
  },
  {
    id: 'scen_comm_1',
    domain: 'communication',
    question:
      'Someone sends you a long message asking for detailed advice. You are busy and can only reply briefly right now. What does your reply look like?',
  },
  {
    id: 'scen_comm_2',
    domain: 'communication',
    question:
      'You need to give negative feedback to someone you like and respect. How do you phrase it?',
  },
  {
    id: 'scen_comm_3',
    domain: 'communication',
    question:
      'In a group chat, someone states an opinion you strongly disagree with. Everyone else agrees with them. Do you respond? What do you write?',
  },
  {
    id: 'scen_product_1',
    domain: 'product',
    question:
      'Your product has two candidate features: one is fast to build and could attract many new users, the other is slower but deepens trust with existing power users. You can only build one this quarter. Which do you choose?',
  },
  {
    id: 'scen_product_2',
    domain: 'product',
    question:
      'A user reports a confusing onboarding flow. Your data says only 5% of users hit the problem, and fixing it would take a week. Do you fix it now or later?',
  },
  {
    id: 'scen_product_3',
    domain: 'product',
    question:
      'You discover that your top-paying customer uses your product in a way you never designed for. Do you adapt the product to them or stay focused on your original vision?',
  },
  {
    id: 'scen_lead_1',
    domain: 'leadership',
    question:
      'Your team misses a deadline. The reason is that you were slow to approve a design decision. How do you handle the retrospective?',
  },
  {
    id: 'scen_lead_2',
    domain: 'leadership',
    question:
      'Two of your best people disagree about the right technical direction and both are credible. The decision affects the next six months. How do you decide?',
  },
  {
    id: 'scen_lead_3',
    domain: 'leadership',
    question:
      'A junior teammate makes a mistake that costs the company money. Nobody else knows it was them. What do you do?',
  },
  {
    id: 'scen_personal_1',
    domain: 'personal',
    question:
      'An old friend asks you to reconnect, but they drain your energy. You have limited free time. How do you respond to their invitation?',
  },
  {
    id: 'scen_personal_2',
    domain: 'personal',
    question:
      'You are invited to two events on the same evening: a large networking party that could help your career, and a quiet dinner with three close friends. Which do you choose?',
  },
  {
    id: 'scen_personal_3',
    domain: 'personal',
    question:
      'You notice a family member repeatedly makes choices you believe are harmful. You have already given advice twice. Do you bring it up a third time?',
  },
  {
    id: 'scen_ethics_1',
    domain: 'ethics',
    question: "You find a wallet with cash and the owner's ID. Nobody saw you. What do you do?",
  },
  {
    id: 'scen_ethics_2',
    domain: 'ethics',
    question:
      'Your employer asks you to report a metric in a way that looks better than it is, without lying outright. How do you react?',
  },
  {
    id: 'scen_ethics_3',
    domain: 'ethics',
    question:
      'A stranger on the internet offers you free access to an expensive tool in exchange for your personal data. It would genuinely help your work. Do you accept?',
  },
];

/** Сценарий по id; сбой невозможен для id из банка. */
export function scenarioById(id: string): BlindScenario | undefined {
  return SCENARIO_BANK.find((s) => s.id === id);
}

/** Все области банка (для группировки в UI), в порядке появления. */
export function scenarioDomains(): string[] {
  const seen: string[] = [];
  for (const s of SCENARIO_BANK) {
    if (!seen.includes(s.domain)) seen.push(s.domain);
  }
  return seen;
}

/** Рандомный сценарий из банка (используется UI). */
export function randomScenario(): BlindScenario {
  const index = Math.floor(Math.random() * SCENARIO_BANK.length);
  const scenario = SCENARIO_BANK[index];
  if (!scenario) throw new Error('scenario bank is empty');
  return scenario;
}

/**
 * B1: короткий вручную проверяемый профиль (§6.3). Детерминированно собирается
 * из активных сущностей: границы и решения выше предпочтений и фактов;
 * restricted исключается; evidence не включается. Пользователь может
 * отредактировать результат — в этом случае используется его текст.
 */
export function buildBaselineProfile(
  entities: ContextEntity[],
  maxLines = 15,
  maxChars = 1400,
): string {
  const TIER: Record<string, number> = {
    boundary: 4,
    decision: 3,
    goal: 2,
    preference: 1,
    fact: 1,
  };
  const LABEL: Record<string, string> = {
    boundary: 'Boundary',
    decision: 'Decided',
    goal: 'Goal',
    preference: 'Prefers',
    fact: 'Fact',
  };

  const eligible = entities
    .filter((e) => e.status === 'active')
    .map((e) => {
      let data: { claim?: unknown; sensitivity?: unknown; confidence?: unknown } = {};
      try {
        const parsed: unknown = JSON.parse(e.data);
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          data = parsed as {
            claim?: unknown;
            sensitivity?: unknown;
            confidence?: unknown;
          };
        }
      } catch {
        data = {};
      }
      const sensitivity = data.sensitivity;
      const restricted = typeof sensitivity === 'string' && sensitivity === 'restricted';
      const claim = typeof data.claim === 'string' ? data.claim.trim() : '';
      const confidence =
        typeof data.confidence === 'number' && Number.isFinite(data.confidence)
          ? data.confidence
          : 0;
      return { e, claim, restricted, confidence, tier: TIER[e.entity_type] ?? 0 };
    })
    .filter((x) => !x.restricted && x.claim.length > 0)
    .sort(
      (a, b) =>
        b.tier - a.tier ||
        b.confidence - a.confidence ||
        b.e.updated_at.localeCompare(a.e.updated_at) ||
        a.e.id.localeCompare(b.e.id),
    )
    .slice(0, maxLines);

  const lines: string[] = [];
  let total = 0;
  for (const item of eligible) {
    const prefix = LABEL[item.e.entity_type] ?? 'Note';
    const line = `${prefix}: ${item.claim}`;
    if (total + line.length + 1 > maxChars) break;
    lines.push(line);
    total += line.length + 1;
  }
  return lines.join('\n');
}

/** Регулярная часть промпта — одинакова для обоих вариантов. */
const ANSWER_RULES =
  'Answer in 2-4 sentences. Plain text, no preamble, no markdown headers, no bullet lists.';

/** Промпт варианта SOUL: контекст компилируется компилятором SESSION-07. */
export function soulPromptFor(input: {
  scenario: BlindScenario;
  name: string;
  pack: ContextPack;
}): string {
  const { scenario, name, pack } = input;
  return [
    `Answer the following question the way ${name} would actually decide.`,
    '',
    `[SOUL CONTEXT]`,
    pack.serialized,
    `[/SOUL CONTEXT]`,
    '',
    `Question: ${scenario.question}`,
    '',
    ANSWER_RULES,
  ].join('\n');
}

/** Промпт базового варианта: только короткий профиль (B1). */
export function baselinePromptFor(input: { scenario: BlindScenario; profile: string }): string {
  const { scenario, profile } = input;
  return [
    `Answer the following question the way a person described by this short profile would decide.`,
    '',
    `[SHORT PROFILE]`,
    profile,
    `[/SHORT PROFILE]`,
    '',
    `Question: ${scenario.question}`,
    '',
    ANSWER_RULES,
  ].join('\n');
}

/** Пак SOUL-контекста для сценария: запрос = текст сценария (§11.2). */
export function compileScenarioPack(
  entities: ContextEntity[],
  scenario: BlindScenario,
): ContextPack {
  return compileContext(entities, {
    text: scenario.question,
    domains: [],
    projects: [],
    people: [],
    channels: [],
    sensitivity: [],
    statuses: ['active'],
    since: null,
    until: null,
    maxTokens: CONTEXT_STANDARD_TOKENS,
  });
}

/** Форма раунда, возвращаемая Rust (EvaluationRow из src-tauri/src/eval.rs). */
export interface EvaluationRecord {
  id: string;
  soul_id: string;
  scenario_id: string;
  scenario_text: string;
  domain: string;
  soul_variant: 'a' | 'b';
  soul_answer: string;
  baseline_answer: string;
  baseline_profile: string;
  context_pack: string;
  context_entity_ids: string[];
  user_choice: 'a' | 'b' | 'neither' | null;
  completed_at: string | null;
  created_at: string;
}

export interface DisplayVariant {
  label: 'A' | 'B';
  text: string;
  isSoul: boolean;
}

/** Два варианта для экрана слепого выбора: слоты A/B с сохранённым слотом SOUL. */
export function displayVariants(record: EvaluationRecord): [DisplayVariant, DisplayVariant] {
  const soulFirst = record.soul_variant === 'a';
  return [
    {
      label: 'A',
      text: soulFirst ? record.soul_answer : record.baseline_answer,
      isSoul: soulFirst,
    },
    {
      label: 'B',
      text: soulFirst ? record.baseline_answer : record.soul_answer,
      isSoul: !soulFirst,
    },
  ];
}

export interface RevealResult {
  matchedSoul: boolean;
  soulLabel: 'A' | 'B';
  choiceLabel: 'A' | 'B' | 'Neither';
}

/** Раскрытие после выбора: выбор против сохранённого слота. */
export function revealFor(record: EvaluationRecord, choice: 'a' | 'b' | 'neither'): RevealResult {
  const choiceLabel = choice === 'a' ? 'A' : choice === 'b' ? 'B' : 'Neither';
  const soulLabel = record.soul_variant === 'a' ? 'A' : 'B';
  return {
    matchedSoul: choice !== 'neither' && choice === record.soul_variant,
    soulLabel,
    choiceLabel,
  };
}

export interface EvalStats {
  /** Все записанные раунды. */
  total: number;
  /** Раунды с выбором. */
  completed: number;
  wins: number;
  losses: number;
  ties: number;
  /** win rate на не-тайных; null, если решать нечего. */
  winRate: number | null;
  winRateLabel: string;
  /** Точный двусторонний биномиальный p-value (p0 = 0.5); null без завершённых. */
  pValue: number | null;
  pValueLabel: string;
  /** Wilson 95% доверительный интервал win rate; null без завершённых. */
  confidence95: [number, number] | null;
  confidenceLabel: string;
}

export function computeEvalStats(records: readonly EvaluationRecord[]): EvalStats {
  let wins = 0;
  let losses = 0;
  let ties = 0;
  for (const r of records) {
    if (!r.user_choice) continue;
    if (r.user_choice === 'neither') ties += 1;
    else if (r.user_choice === r.soul_variant) wins += 1;
    else losses += 1;
  }
  const decided = wins + losses;
  const winRate = decided > 0 ? wins / decided : null;
  const pValue = decided > 0 ? exactBinomialTwoSided(wins, decided) : null;
  const ci = decided > 0 ? wilson95(wins, decided) : null;
  return {
    total: records.length,
    completed: wins + losses + ties,
    wins,
    losses,
    ties,
    winRate,
    winRateLabel: winRate === null ? '—' : `${(winRate * 100).toFixed(1)}%`,
    pValue,
    pValueLabel: formatPValue(pValue),
    confidence95: ci,
    confidenceLabel:
      ci === null ? '—' : `${(ci[0] * 100).toFixed(1)}%–${(ci[1] * 100).toFixed(1)}%`,
  };
}

function formatPValue(p: number | null): string {
  if (p === null) return '—';
  return p < 0.001 ? '<0.001' : p.toFixed(4);
}

/**
 * Точный двусторонний биномиальный тест для p0 = 0.5 на не-тайных
 * (wins + losses). Распределение симметрично: p = 2 * P(X <= min(wins, losses)).
 */
export function exactBinomialTwoSided(wins: number, total: number): number {
  if (total <= 0) return 1;
  const minSide = Math.min(wins, total - wins);
  let cumulative = 0;
  for (let k = 0; k <= minSide; k++) {
    cumulative += binomialCoefficient(total, k);
  }
  return Math.min(1, (2 * cumulative) / Math.pow(2, total));
}

function binomialCoefficient(n: number, k: number): number {
  if (k < 0 || k > n) return 0;
  let value = 1;
  for (let i = 0; i < k; i++) {
    value = (value * (n - i)) / (i + 1);
  }
  return value;
}

/**
 * Wilson score interval, 95% (z = 1.96). Интервал по §6.6; для 35/44 даёт
 * 65.5%–88.8% — контрольный вектор из примера мастера.
 */
export function wilson95(wins: number, total: number): [number, number] {
  if (total <= 0) return [0, 0];
  const z = 1.959963984540054;
  const p = wins / total;
  const z2 = z * z;
  const denom = 1 + z2 / total;
  const center = (p + z2 / (2 * total)) / denom;
  const half = (z * Math.sqrt((p * (1 - p)) / total + z2 / (4 * total * total))) / denom;
  return [Math.max(0, center - half), Math.min(1, center + half)];
}

/**
 * Share-карта (§6.6): только при N >= SHARE_MIN_ROUNDS; без личных вопросов
 * и ответов — только агрегаты, метаданные и честная оговорка.
 */
export function shareCardText(stats: EvalStats, name: string): string | null {
  if (stats.completed < SHARE_MIN_ROUNDS) return null;
  const ci = stats.confidence95;
  const lines = [
    'SOUL BLIND TEST',
    '',
    `Rounds: ${stats.completed} (${stats.wins + stats.losses} decided)`,
    `SOUL wins: ${stats.wins}`,
    `Baseline wins: ${stats.losses}`,
    `Neither: ${stats.ties}`,
    '',
    `Win rate: ${stats.winRateLabel}`,
    ci ? `95% CI: ${stats.confidenceLabel}` : '95% CI: —',
    `Exact binomial p: ${stats.pValueLabel}`,
    '',
    `Model: same for both variants`,
    `SOUL: ${name}`,
  ];
  return lines.join('\n');
}
