import { describe, it, expect } from 'vitest';
import {
  SoulEntityType,
  EntityStatus,
  Sensitivity,
  SoulEntity,
  PreferenceEntity,
  DecisionEntity,
  BoundaryEntity,
  AnyEntitySchema,
} from 'soul-schema';

describe('SoulEntityType', () => {
  it('validates known entity types', () => {
    expect(SoulEntityType.parse('preference')).toBe('preference');
    expect(SoulEntityType.parse('decision')).toBe('decision');
    expect(SoulEntityType.parse('boundary')).toBe('boundary');
    expect(SoulEntityType.parse('goal')).toBe('goal');
    expect(SoulEntityType.parse('fact')).toBe('fact');
  });

  it('rejects unknown entity type', () => {
    expect(() => SoulEntityType.parse('memory')).toThrow();
  });
});

describe('EntityStatus', () => {
  it('validates candidate', () => {
    expect(EntityStatus.parse('candidate')).toBe('candidate');
  });
  it('validates active', () => {
    expect(EntityStatus.parse('active')).toBe('active');
  });
});

describe('Sensitivity', () => {
  it('validates all levels', () => {
    expect(Sensitivity.parse('public')).toBe('public');
    expect(Sensitivity.parse('restricted')).toBe('restricted');
  });
});

describe('SoulEntity', () => {
  const baseEntity = {
    id: 'ent_01',
    type: 'preference',
    namespace: 'user.test',
    subject: 'test entity',
    status: 'active',
    scope: { domains: [], projects: [], people: [], channels: [] },
    confidence: 0.8,
    importance: 0.5,
    sensitivity: 'private',
    stability: 'stable',
    validFrom: null,
    validUntil: null,
    evidenceIds: [],
    supersedes: [],
    conflictsWith: [],
    createdAt: '2026-07-30T12:00:00Z',
    updatedAt: '2026-07-30T12:00:00Z',
  };

  it('validates a valid preference entity', () => {
    const pref = { ...baseEntity, value: 'short answers', strength: 0.9, exceptions: [], alternatives: [] };
    expect(() => PreferenceEntity.parse(pref)).not.toThrow();
  });

  it('validates a valid decision entity', () => {
    const decision = {
      ...baseEntity, type: 'decision', question: 'Which stack?',
      options: ['React', 'Svelte'], selected: 'React',
      reasons: ['ecosystem'], rejectedReasons: [],
      conditionsThatWouldChangeDecision: [], outcome: null,
    };
    expect(() => DecisionEntity.parse(decision)).not.toThrow();
  });

  it('validates a valid boundary entity', () => {
    const boundary = {
      ...baseEntity, type: 'boundary', hardness: 'hard',
      actionKinds: ['purchase.create'], effect: 'deny',
    };
    expect(() => BoundaryEntity.parse(boundary)).not.toThrow();
  });

  it('rejects confidence outside 0-1', () => {
    expect(() => SoulEntity.parse({ ...baseEntity, confidence: 1.5 })).toThrow();
  });

  it('discriminated union parses preference', () => {
    const pref = { ...baseEntity, value: 'x', strength: 0.5, exceptions: [], alternatives: [] };
    expect(AnyEntitySchema.parse(pref).type).toBe('preference');
  });
});
