import { describe, it, expect } from 'vitest';
import { CALIBRATION_STEPS, type CalibrationAnswer } from '../src/data/calibration';
import { compileAnswers, type P0EntityType } from '../src/data/compile';
import { requiresExplicitConfirm, type ReviewEntity } from '../src/data/review';

const QUESTIONS = CALIBRATION_STEPS.flatMap((s) => s.questions);

function fullAnswers(): CalibrationAnswer[] {
  return [
    { questionId: 'pref_1', value: 'Concise' },
    { questionId: 'pref_2', value: 'Bullet points' },
    { questionId: 'goal_1', value: 'Build a product' },
    { questionId: 'bound_1', value: 'Financial decisions' },
    { questionId: 'bound_2', value: 'Passwords and secrets' },
    { questionId: 'dec_1', value: 'Documentation quality' },
    { questionId: 'text_1', value: 'I build desktop apps.' },
    { questionId: 'write_1', value: 'Short writing sample.' },
  ];
}

const P0_TYPES: readonly P0EntityType[] = ['preference', 'decision', 'boundary', 'goal', 'fact'];

describe('compileAnswers', () => {
  it('maps every calibration answer to one of the P0 entity types', () => {
    const compiled = compileAnswers(fullAnswers(), QUESTIONS);
    expect(compiled.length).toBeGreaterThanOrEqual(7);
    for (const item of compiled) {
      expect(P0_TYPES).toContain(item.type);
    }
  });

  it('attaches source id, scope, confidence, sensitivity and explicitness to every item', () => {
    const compiled = compileAnswers(fullAnswers(), QUESTIONS);
    expect(compiled.length).toBeGreaterThan(0);
    for (const item of compiled) {
      expect(item.questionId.length).toBeGreaterThan(0);
      expect(item.data.source).toBe('calibration');
      expect(item.data.questionId).toBe(item.questionId);
      expect(item.data.confidence).toBeGreaterThanOrEqual(0);
      expect(item.data.confidence).toBeLessThanOrEqual(1);
      expect(item.data.explicitness).toBeGreaterThanOrEqual(0);
      expect(item.data.explicitness).toBeLessThanOrEqual(1);
      expect(item.data.scope.domains.length).toBeGreaterThan(0);
      expect(item.data.claim.length).toBeGreaterThan(0);
    }
  });

  it('skips unknown question ids and unsupported categories', () => {
    const compiled = compileAnswers(
      [
        { questionId: 'pref_1', value: 'Concise' },
        { questionId: 'unknown_q', value: 'x' },
      ],
      QUESTIONS,
    );
    expect(compiled).toHaveLength(1);
    expect(compiled[0]!.questionId).toBe('pref_1');
  });

  it('keeps each item linked to its concrete user answer', () => {
    const compiled = compileAnswers(fullAnswers(), QUESTIONS);
    const pref = compiled.find((c) => c.questionId === 'pref_1');
    expect(pref).toBeDefined();
    expect(pref!.data.claim).toContain('Concise');
    const goal = compiled.find((c) => c.questionId === 'goal_1');
    expect(goal!.type).toBe('goal');
    const boundary = compiled.find((c) => c.questionId === 'bound_1');
    expect(boundary!.type).toBe('boundary');
  });

  it('marks sensitive boundaries as requiring individual confirmation', () => {
    const compiled = compileAnswers(fullAnswers(), QUESTIONS);
    const boundary = compiled.find((c) => c.questionId === 'bound_1')!;
    expect(boundary.data.risk).toBe(true);
    const entity: ReviewEntity = {
      id: 'x',
      entity_type: boundary.type,
      status: 'candidate',
      data: JSON.stringify(boundary.data),
      created_at: '2026-07-31T10:00:00Z',
    };
    expect(requiresExplicitConfirm(entity)).toBe(true);
  });

  it('flags a disputed combination when nothing is off-limits but topics are chosen', () => {
    const answers: CalibrationAnswer[] = [
      { questionId: 'bound_1', value: 'Financial decisions' },
      { questionId: 'bound_2', value: 'Nothing is off-limits' },
    ];
    const compiled = compileAnswers(answers, QUESTIONS);
    expect(compiled).toHaveLength(2);
    for (const item of compiled) {
      expect(item.disputed).toBe(true);
      expect(item.data.disputed).toBe(true);
    }
  });

  it('does not flag disputes when nothing is off-limits is not selected', () => {
    const compiled = compileAnswers(fullAnswers(), QUESTIONS);
    for (const item of compiled) {
      expect(item.disputed).toBe(false);
    }
  });

  it('does not mark a disputed non-boundary preference', () => {
    const compiled = compileAnswers([{ questionId: 'pref_1', value: 'Concise' }], QUESTIONS);
    expect(compiled[0]!.disputed).toBe(false);
  });

  it('returns an empty list for empty answers', () => {
    expect(compileAnswers([], QUESTIONS)).toEqual([]);
  });

  it('is idempotent and fully deterministic (zero tokens on this path)', () => {
    const answers = fullAnswers();
    const first = JSON.stringify(compileAnswers(answers, QUESTIONS));
    for (let i = 0; i < 100; i++) {
      expect(JSON.stringify(compileAnswers(answers, QUESTIONS))).toBe(first);
    }
  });

  it('does not mutate the input answers', () => {
    const answers = fullAnswers();
    const snapshot = JSON.stringify(answers);
    compileAnswers(answers, QUESTIONS);
    expect(JSON.stringify(answers)).toBe(snapshot);
  });
});
