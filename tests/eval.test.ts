import { describe, it, expect, vi } from 'vitest';
import {
  SCENARIO_BANK,
  scenarioById,
  scenarioDomains,
  buildBaselineProfile,
  soulPromptFor,
  baselinePromptFor,
  compileScenarioPack,
  computeEvalStats,
  exactBinomialTwoSided,
  wilson95,
  displayVariants,
  revealFor,
  shareCardText,
  EVAL_RECOMMENDED_ROUNDS,
  SHARE_MIN_ROUNDS,
  B1_PROFILE_STORAGE_KEY,
  clearPersistedBaselineProfile,
  type EvaluationRecord,
} from '../src/data/eval';
import type { ContextEntity } from '../src/data/context';

function entity(overrides: Partial<ContextEntity> & { id: string }): ContextEntity {
  return {
    soul_id: 'soul_t',
    entity_type: 'preference',
    status: 'active',
    data: '{"claim":"Prefer concise","sensitivity":"internal"}',
    created_at: '2026-07-30T10:00:00Z',
    updated_at: '2026-07-30T10:00:00Z',
    ...overrides,
  };
}

function record(overrides: Partial<EvaluationRecord>): EvaluationRecord {
  return {
    id: 'evl_1',
    soul_id: 'soul_t',
    scenario_id: 'scen_1',
    scenario_text: 'Q?',
    domain: 'career',
    soul_variant: 'a',
    soul_answer: 'soul answer',
    baseline_answer: 'baseline answer',
    baseline_profile: 'profile',
    context_pack: 'pack',
    context_entity_ids: [],
    user_choice: null,
    completed_at: null,
    created_at: '2026-08-01T00:00:00Z',
    ...overrides,
  };
}

describe('SCENARIO_BANK', () => {
  it('содержит не меньше 20 сценариев с уникальными id и непустыми вопросами', () => {
    expect(SCENARIO_BANK.length).toBeGreaterThanOrEqual(EVAL_RECOMMENDED_ROUNDS);
    const ids = new Set(SCENARIO_BANK.map((s) => s.id));
    expect(ids.size).toBe(SCENARIO_BANK.length);
    for (const s of SCENARIO_BANK) {
      expect(s.question.trim().length).toBeGreaterThan(20);
      expect(s.domain.trim().length).toBeGreaterThan(0);
    }
  });

  it('scenarioById и scenarioDomains работают детерминированно', () => {
    const first = SCENARIO_BANK[0];
    if (!first) throw new Error('bank empty');
    expect(scenarioById(first.id)?.question).toBe(first.question);
    expect(scenarioById('scen_nope')).toBeUndefined();
    const domains = scenarioDomains();
    expect(domains.length).toBeGreaterThan(3);
    expect(new Set(domains).size).toBe(domains.length);
  });
});

describe('buildBaselineProfile', () => {
  it('пропускает sensitive/restricted, ставит границы выше предпочтений, режет по лимиту', () => {
    const boundary = entity({
      id: 'b1',
      entity_type: 'boundary',
      data: '{"claim":"Never send money to strangers","sensitivity":"internal"}',
    });
    const restricted = entity({
      id: 'r1',
      data: '{"claim":"Secret health detail","sensitivity":"restricted"}',
    });
    const pref = entity({ id: 'p1' });
    const sensitive = entity({
      id: 's1',
      data: '{"claim":"Private medical detail","sensitivity":"sensitive"}',
    });
    const candidate = entity({
      id: 'c1',
      status: 'candidate',
    });
    const text = buildBaselineProfile([pref, restricted, sensitive, candidate, boundary], 15, 1400);
    const lines = text.split('\n');
    expect(lines[0]).toContain('Boundary: Never send money');
    expect(text).not.toContain('Secret health detail');
    expect(text).not.toContain('Private medical detail');
    expect(text).toContain('Prefers: Prefer concise');
    expect(lines.length).toBeLessThanOrEqual(3);
  });

  it('clears the legacy localStorage B1 profile', () => {
    const removeItem = vi.fn();
    clearPersistedBaselineProfile({ removeItem });
    expect(removeItem).toHaveBeenCalledWith(B1_PROFILE_STORAGE_KEY);
  });

  it('пустой вход даёт пустой профиль', () => {
    expect(buildBaselineProfile([], 15, 1400)).toBe('');
  });

  it('лимит символов работает', () => {
    const many = Array.from({ length: 20 }, (_, i) =>
      entity({ id: `e${i}`, data: `{"claim":"Claim number ${i} with some padding text","sensitivity":"public"}` }),
    );
    const text = buildBaselineProfile(many, 15, 60);
    expect(text.length).toBeLessThanOrEqual(60);
    expect(text.length).toBeGreaterThan(0);
  });
});

describe('промпты вариантов', () => {
  const scenario = { id: 'scen_t', domain: 'career', question: 'Which offer do you take?' };
  const pack = compileScenarioPack([entity({ id: 'p1' })], scenario);

  it('SOUL-промпт содержит пак и вопрос, правила ответа одинаковы', () => {
    const soul = soulPromptFor({ scenario, name: 'Alex', pack });
    expect(soul).toContain('Which offer do you take?');
    expect(soul).toContain(pack.serialized);
    expect(soul).toContain('Alex');
  });

  it('базовый промпт содержит профиль, но не пак', () => {
    const base = baselinePromptFor({ scenario, profile: 'Prefers: fast shipping' });
    expect(base).toContain('Prefers: fast shipping');
    expect(base).not.toContain('SOUL CONTEXT');
  });

  it('оба промпта заканчиваются одинаковыми правилами ответа', () => {
    const soul = soulPromptFor({ scenario, name: 'Alex', pack });
    const base = baselinePromptFor({ scenario, profile: 'p' });
    const rules = 'Answer in 2-4 sentences.';
    expect(soul).toContain(rules);
    expect(base).toContain(rules);
  });
});

describe('статистика', () => {
  it('Wilson 95% для 35/44 = 65.5%–88.8% (контрольный вектор мастера)', () => {
    const [lo, hi] = wilson95(35, 44);
    expect(lo).toBeCloseTo(0.65499, 3);
    expect(hi).toBeCloseTo(0.88847, 3);
    expect(35 / 44).toBeCloseTo(0.795455, 3);
  });

  it('точный бином: 14/20 → 2*60460/2^20', () => {
    const expected = (2 * 60460) / 1048576;
    expect(exactBinomialTwoSided(14, 20)).toBeCloseTo(expected, 6);
  });

  it('точный бином: 10/10 → 1/512', () => {
    expect(exactBinomialTwoSided(10, 10)).toBeCloseTo(1 / 512, 9);
  });

  it('точный бином симметричен: (9,44) = (35,44)', () => {
    expect(exactBinomialTwoSided(9, 44)).toBeCloseTo(exactBinomialTwoSided(35, 44), 12);
    expect(exactBinomialTwoSided(35, 44)).toBeLessThan(0.001);
  });

  it('пример из мастера: 48 раундов, 35 побед, 9 поражений, 4 ties', () => {
    const rounds: EvaluationRecord[] = [
      ...Array.from({ length: 35 }, (_, i) =>
        record({ id: `w${i}`, user_choice: 'a', soul_variant: 'a' }),
      ),
      ...Array.from({ length: 9 }, (_, i) =>
        record({ id: `l${i}`, user_choice: 'b', soul_variant: 'a' }),
      ),
      ...Array.from({ length: 4 }, (_, i) =>
        record({ id: `t${i}`, user_choice: 'neither' }),
      ),
    ];
    const stats = computeEvalStats(rounds);
    expect(stats.total).toBe(48);
    expect(stats.completed).toBe(48);
    expect(stats.wins).toBe(35);
    expect(stats.losses).toBe(9);
    expect(stats.ties).toBe(4);
    expect(stats.winRate).toBeCloseTo(0.795455, 3);
    expect(stats.winRateLabel).toBe('79.5%');
    expect(stats.confidence95?.[0]).toBeCloseTo(0.65499, 3);
    expect(stats.confidence95?.[1]).toBeCloseTo(0.88847, 3);
    expect(stats.pValue ?? 1).toBeLessThan(0.001);
  });

  it('незавершённые раунды не считаются; пусто — нули', () => {
    const stats = computeEvalStats([record({}), record({ user_choice: 'neither' })]);
    expect(stats.total).toBe(2);
    expect(stats.completed).toBe(1);
    expect(stats.wins).toBe(0);
    expect(stats.losses).toBe(0);
    expect(stats.ties).toBe(1);
    expect(stats.winRate).toBeNull();
    expect(stats.pValue).toBeNull();
    expect(stats.confidence95).toBeNull();
    expect(stats.winRateLabel).toBe('—');
  });
});

describe('слепой выбор и раскрытие', () => {
  it('displayVariants показывает ответы по слотам без знания слота', () => {
    const r = record({ soul_variant: 'b', soul_answer: 'S', baseline_answer: 'B' });
    const [a, b] = displayVariants(r);
    expect(a.text).toBe('B');
    expect(b.text).toBe('S');
    expect(b.isSoul).toBe(true);
    expect(a.isSoul).toBe(false);
  });

  it('revealFor: выбор == слот → победа SOUL; neither → не победа', () => {
    const r = record({ soul_variant: 'b', soul_answer: 'S', baseline_answer: 'B' });
    const hit = revealFor(r, 'b');
    expect(hit.matchedSoul).toBe(true);
    expect(hit.soulLabel).toBe('B');
    expect(hit.choiceLabel).toBe('B');
    expect(revealFor(r, 'a').matchedSoul).toBe(false);
    const tie = revealFor(r, 'neither');
    expect(tie.matchedSoul).toBe(false);
    expect(tie.choiceLabel).toBe('Neither');
  });
});

describe('share-карта', () => {
  it('меньше минимума раундов — не публикуется', () => {
    const stats = computeEvalStats(
      Array.from({ length: 5 }, () => record({ user_choice: 'a' })),
    );
    expect(shareCardText(stats, 'Alex')).toBeNull();
  });

  it('от 20 раундов — карта с агрегатами без личного содержимого', () => {
    const rounds: EvaluationRecord[] = Array.from({ length: SHARE_MIN_ROUNDS }, (_, i) =>
      record({
        id: `r${i}`,
        user_choice: i < 14 ? 'a' : 'b',
        soul_variant: 'a',
        scenario_text: 'private dilemma',
      }),
    );
    const stats = computeEvalStats(rounds);
    const card = shareCardText(stats, 'Alex');
    expect(card).not.toBeNull();
    expect(card).toContain('SOUL BLIND TEST');
    expect(card).toContain('SOUL wins: 14');
    expect(card).toContain('Baseline wins: 6');
    expect(card).toContain('Win rate: 70.0%');
    expect(card).not.toContain('Model:');
    expect(card).not.toContain('same for both variants');
    expect(card).not.toContain('private dilemma');
    expect(card).not.toContain('soul answer');
  });
});
