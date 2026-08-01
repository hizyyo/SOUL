import { describe, it, expect } from 'vitest';
import {
  EFFECT_RANK,
  POLICY_PRESETS,
  presetById,
  validateRuleJson,
  effectOfRuleJson,
  effectLabel,
  EVALUATION_EXAMPLE,
  MAX_PRIORITY,
  type Effect,
  type PolicyRow,
} from '../src/data/policy';

describe('effect lattice (§12.3)', () => {
  it('deny beats require_confirmation beats redact beats allow', () => {
    expect(EFFECT_RANK.deny).toBeGreaterThan(EFFECT_RANK.require_confirmation);
    expect(EFFECT_RANK.require_confirmation).toBeGreaterThan(EFFECT_RANK.redact);
    expect(EFFECT_RANK.redact).toBeGreaterThan(EFFECT_RANK.allow);
  });

  it('labels every effect in Russian', () => {
    for (const effect of Object.keys(EFFECT_RANK) as Effect[]) {
      expect(effectLabel(effect).length).toBeGreaterThan(0);
    }
  });
});

describe('presets', () => {
  it('builds at least the two seed rules', () => {
    expect(presetById('policy_high_value_confirmation')).toBeDefined();
    expect(presetById('policy_destructive_denied')).toBeDefined();
  });

  it('every preset is a valid SoulRule JSON', () => {
    for (const preset of POLICY_PRESETS) {
      const json = preset.build();
      expect(validateRuleJson(json).ok).toBe(true);
      const parsed = JSON.parse(json) as Record<string, unknown>;
      expect(parsed.id).toBe(preset.id);
      expect(parsed.when).toBeDefined();
    }
  });

  it('unknown preset id resolves to undefined', () => {
    expect(presetById('no_such_preset')).toBeUndefined();
  });

  it('high-value preset checks kind and amount > 500', () => {
    const parsed = JSON.parse(
      presetById('policy_high_value_confirmation')?.build() ?? '{}',
    ) as { when: { all: unknown[] } };
    expect(parsed.when.all).toHaveLength(2);
  });
});

describe('validateRuleJson', () => {
  const good = (): string =>
    JSON.stringify({
      id: 'r1',
      priority: 100,
      when: { eq: ['action.kind', 'purchase.create'] },
      effect: 'allow',
    });

  it('accepts a valid rule', () => {
    const state = validateRuleJson(good());
    expect(state.ok).toBe(true);
    expect(state.error).toBeNull();
    expect(state.priority).toBe(100);
    expect(state.effect).toBe('allow');
  });

  it('rejects invalid JSON', () => {
    const state = validateRuleJson('{not json');
    expect(state.ok).toBe(false);
    expect(state.error).toMatch(/not valid JSON/);
  });

  it('rejects non-object JSON', () => {
    expect(validateRuleJson('[]').ok).toBe(false);
    expect(validateRuleJson('42').ok).toBe(false);
  });

  it('rejects empty or missing id', () => {
    expect(validateRuleJson(good().replace('"r1"', '""')).ok).toBe(false);
    expect(validateRuleJson(good().replace('"id":"r1",', '')).ok).toBe(false);
  });

  it('rejects unknown effect', () => {
    const bad = good().replace('"allow"', '"explode"');
    expect(validateRuleJson(bad).ok).toBe(false);
  });

  it('rejects priority out of range and non-integers', () => {
    expect(validateRuleJson(good().replace('100', '100.5')).ok).toBe(false);
    expect(validateRuleJson(good().replace('100', '-1')).ok).toBe(false);
    expect(validateRuleJson(good().replace('100', String(MAX_PRIORITY + 1))).ok).toBe(false);
    expect(validateRuleJson(good().replace('100', String(MAX_PRIORITY))).ok).toBe(true);
  });
});

describe('effectOfRuleJson', () => {
  it('extracts effect from a rule row', () => {
    const row: Pick<PolicyRow, 'rule_json'> = {
      rule_json: JSON.stringify({ id: 'r1', priority: 5, effect: 'deny' }),
    };
    expect(effectOfRuleJson(row.rule_json)).toBe('deny');
  });

  it('returns null for broken JSON or missing effect', () => {
    expect(effectOfRuleJson('{oops')).toBeNull();
    expect(effectOfRuleJson(JSON.stringify({ id: 'r1' }))).toBeNull();
  });
});

describe('evaluation example', () => {
  it('is parseable and complete for the demo gateway (§4.11)', () => {
    const action = JSON.parse(EVALUATION_EXAMPLE) as Record<string, unknown>;
    expect(action.kind).toBe('purchase.create');
    expect(action.amount).toBe(600);
    expect(action.environment).toBe('production');
  });
});
