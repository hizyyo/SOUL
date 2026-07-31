import { describe, it, expect } from 'vitest';
import {
  compileContext,
  defaultQuery,
  detectConflicts,
  dedupeSuperseded,
  estimateTokens,
  collectDomains,
  CONTEXT_STANDARD_TOKENS,
  CONTEXT_HARD_MAX_TOKENS,
  type ContextEntity,
  type ContextQuery,
} from '../src/data/context';

let seq = 0;
function entity(over: Partial<ContextEntity> & { data?: string } = {}): ContextEntity {
  seq += 1;
  return {
    id: `ent-${String(seq).padStart(4, '0')}`,
    soul_id: 'soul_test',
    entity_type: 'preference',
    status: 'active',
    data:
      `{"claim":"Test claim","evidence":"Test evidence","questionId":"pref_${String(seq).padStart(4, '0')}",` +
      '"kind":"binary","value":"yes","confidence":0.9,"explicitness":1,"sensitivity":"internal",' +
      '"scope":{"domains":["preferences"],"projects":[],"people":[],"channels":[]},"risk":false}',
    created_at: '2026-07-01T10:00:00.000Z',
    updated_at: '2026-07-01T10:00:00.000Z',
    ...over,
  };
}

function entityData(data: string, over: Partial<ContextEntity> = {}): ContextEntity {
  return entity({ data, ...over });
}

function dataFor(over: Record<string, unknown> = {}): string {
  return JSON.stringify({
    claim: 'Test claim',
    evidence: 'Test evidence',
    questionId: 'pref_001',
    kind: 'binary',
    value: 'yes',
    confidence: 0.9,
    explicitness: 1,
    sensitivity: 'internal',
    scope: { domains: ['preferences'], projects: [], people: [], channels: [] },
    risk: false,
    ...over,
  });
}

function query(over: Partial<ContextQuery> = {}): ContextQuery {
  return { ...defaultQuery(), ...over };
}

describe('estimateTokens', () => {
  it('counts CJK characters as one token each', () => {
    expect(estimateTokens('编程')).toBe(2);
    expect(estimateTokens('中文测试')).toBe(4);
  });

  it('counts latin text at about 1/3 token per char, never zero', () => {
    expect(estimateTokens('')).toBe(0);
    expect(estimateTokens('a')).toBe(1);
    expect(estimateTokens('prefer concise technical answers')).toBe(11);
  });
});

describe('relevance and text filter', () => {
  it('keeps only entities matching the query text', () => {
    const a = entityData(dataFor({ claim: 'Prefers concise technical answers' }));
    const b = entityData(dataFor({ claim: 'Always delegates phone calls' }));
    const pack = compileContext([a, b], query({ text: 'concise technical' }));
    expect(pack.items.map((i) => i.id)).toEqual([a.id]);
  });

  it('empty query keeps everything eligible (no text gate)', () => {
    const a = entityData(dataFor({ claim: 'Anything', questionId: 'pref_qa' }));
    const b = entityData(dataFor({ claim: 'Everything', questionId: 'pref_qb' }));
    const pack = compileContext([a, b], query({}));
    expect(pack.items).toHaveLength(2);
  });
});

describe('sensitivity gate', () => {
  it('excludes restricted entities unless explicitly allowed', () => {
    const secret = entityData(dataFor({ sensitivity: 'restricted', questionId: 'pref_sec' }));
    const normal = entityData(
      dataFor({ sensitivity: 'private', questionId: 'pref_norm' }),
      { updated_at: '2026-07-02T00:00:00Z' },
    );
    const pack = compileContext([secret, normal], query({}));
    expect(pack.items.map((i) => i.id)).toEqual([normal.id]);
  });

  it('includes restricted when allowed by query', () => {
    const secret = entityData(dataFor({ sensitivity: 'restricted' }));
    const pack = compileContext([secret], query({ sensitivity: ['public', 'internal', 'private', 'sensitive', 'restricted'] }));
    expect(pack.items.map((i) => i.id)).toEqual([secret.id]);
  });
});

describe('scope filters', () => {
  it('project filter never leaks entities from other projects', () => {
    const keep = entityData(
      dataFor({ scope: { domains: ['preferences'], projects: ['SOUL'], people: [], channels: [] } }),
    );
    const other = entityData(
      dataFor({ scope: { domains: ['preferences'], projects: ['NIMBUS'], people: [], channels: [] } }),
    );
    const pack = compileContext([keep, other], query({ projects: ['SOUL'] }));
    expect(pack.items.map((i) => i.id)).toEqual([keep.id]);
  });

  it('people filter matches scope.people', () => {
    const keep = entityData(
      dataFor({ scope: { domains: ['preferences'], projects: [], people: ['alice'], channels: [] } }),
    );
    const other = entityData(
      dataFor({ scope: { domains: ['preferences'], projects: [], people: ['bob'], channels: [] } }),
    );
    const pack = compileContext([keep, other], query({ people: ['alice'] }));
    expect(pack.items.map((i) => i.id)).toEqual([keep.id]);
  });

  it('empty scope dimension means no restriction', () => {
    const a = entityData(dataFor({ questionId: 'pref_x', scope: { domains: ['preferences'], projects: ['X'], people: [], channels: [] } }));
    const b = entityData(dataFor({ questionId: 'pref_y', scope: { domains: ['preferences'], projects: ['Y'], people: [], channels: [] } }));
    const pack = compileContext([a, b], query({}));
    expect(pack.items).toHaveLength(2);
  });
});

describe('status gate', () => {
  it('only active by default', () => {
    const active = entity({ status: 'active' });
    const disputed = entity({ status: 'disputed' });
    const superseded = entity({ status: 'superseded' });
    const deleted = entity({ status: 'deleted' });
    const pack = compileContext([active, disputed, superseded, deleted], query({}));
    expect(pack.items.map((i) => i.id)).toEqual([active.id]);
  });

  it('allows explicit statuses', () => {
    const disputed = entity({ status: 'disputed' });
    const pack = compileContext([disputed], query({ statuses: ['disputed'] }));
    expect(pack.items.map((i) => i.id)).toEqual([disputed.id]);
  });
});

describe('dedupe and conflicts', () => {
  it('dedupes superseded answers: only newest per question survives', () => {
    const oldAnswer = entityData(dataFor({ questionId: 'pref_speed', value: 'fast' }), {
      updated_at: '2026-06-01T00:00:00Z',
    });
    const newAnswer = entityData(dataFor({ questionId: 'pref_speed', value: 'slow' }), {
      updated_at: '2026-07-01T00:00:00Z',
    });
    const { kept, supersededIds } = dedupeSuperseded([oldAnswer, newAnswer]);
    expect(kept.map((e) => e.id)).toEqual([newAnswer.id]);
    expect(supersededIds).toEqual([oldAnswer.id]);
  });

  it('exposes conflicting answers as explicit conflicts', () => {
    const oldAnswer = entityData(dataFor({ questionId: 'pref_speed', value: 'fast' }), {
      updated_at: '2026-06-01T00:00:00Z',
    });
    const newAnswer = entityData(dataFor({ questionId: 'pref_speed', value: 'slow' }), {
      updated_at: '2026-07-01T00:00:00Z',
    });
    const pack = compileContext([oldAnswer, newAnswer], query({}));
    expect(pack.conflicts).toHaveLength(1);
    const conflict = pack.conflicts[0]!;
    expect([conflict.a, conflict.b].sort()).toEqual([oldAnswer.id, newAnswer.id].sort());
    expect(conflict.reason).toContain('pref_speed');
    expect(pack.supersededIds).toContain(oldAnswer.id);
    expect(pack.serialized).toContain('CONFLICTS:');
  });

  it('same answer twice is not a conflict', () => {
    const one = entityData(dataFor({ questionId: 'pref_x', value: 'same', updated_at: '2026-06-01T00:00:00Z' }));
    const two = entityData(dataFor({ questionId: 'pref_x', value: 'same', updated_at: '2026-07-01T00:00:00Z' }));
    expect(detectConflicts([one, two])).toHaveLength(0);
    const pack = compileContext([one, two], query({}));
    expect(pack.conflicts).toHaveLength(0);
  });

  it('legacy entities without questionId are never dropped', () => {
    const legacy = entityData(
      '{"claim":"Legacy","evidence":"","kind":"text","value":"x","confidence":0.5,"explicitness":0.5,"sensitivity":"internal","scope":{"domains":[],"projects":[],"people":[],"channels":[]},"risk":false}',
    );
    const pack = compileContext([legacy], query({}));
    expect(pack.items.map((i) => i.id)).toEqual([legacy.id]);
  });
});

describe('time window', () => {
  it('excludes entities outside [since, until]', () => {
    const inside = entity({ created_at: '2026-07-15T00:00:00Z' });
    const tooOld = entity({ created_at: '2026-01-01T00:00:00Z' });
    const tooNew = entity({ created_at: '2026-08-01T00:00:00Z' });
    const pack = compileContext(
      [inside, tooOld, tooNew],
      query({ since: '2026-07-01T00:00:00Z', until: '2026-07-31T00:00:00Z' }),
    );
    expect(pack.items.map((i) => i.id)).toEqual([inside.id]);
  });

  it('ignores unparseable dates (behaves as always-visible)', () => {
    const weird = entity({ created_at: 'not-a-date' });
    const pack = compileContext([weird], query({ since: '2026-07-01T00:00:00Z' }));
    expect(pack.items.map((i) => i.id)).toEqual([weird.id]);
  });
});

describe('boundaries outrank preferences and facts', () => {
  it('sorts boundaries first, then decisions, goals, preferences/facts', () => {
    const preference = entity({ entity_type: 'preference' });
    const boundary = entity({ entity_type: 'boundary' });
    const fact = entity({ entity_type: 'fact' });
    const decision = entity({ entity_type: 'decision' });
    const pack = compileContext([preference, boundary, fact, decision], query({}));
    expect(pack.items.map((i) => i.entityType)).toEqual(['boundary', 'decision', 'preference', 'fact']);
  });
});

describe('budget packing', () => {
  function longEntity(i: number): ContextEntity {
    const claim = `Long preference number ${i}: ${'padding '.repeat(60)}`.trim();
    return entityData(dataFor({ claim, questionId: `pref_long_${i}` }));
  }

  it('never exceeds the soft budget', () => {
    const pack = compileContext(
      Array.from({ length: 40 }, (_, i) => longEntity(i)),
      query({ maxTokens: CONTEXT_STANDARD_TOKENS }),
    );
    expect(pack.tokenEstimate).toBeLessThanOrEqual(CONTEXT_STANDARD_TOKENS);
    expect(pack.items.length).toBeGreaterThan(0);
    expect(pack.items.length).toBeLessThan(40);
  });

  it('never exceeds the hard maximum even when asked for 3000', () => {
    const pack = compileContext(
      Array.from({ length: 60 }, (_, i) => longEntity(i)),
      query({ maxTokens: CONTEXT_HARD_MAX_TOKENS }),
    );
    expect(pack.tokenEstimate).toBeLessThanOrEqual(CONTEXT_HARD_MAX_TOKENS);
  });

  it('clamps absurd budgets into the legal range', () => {
    const huge = compileContext([longEntity(1)], query({ maxTokens: 99999 }));
    expect(huge.maxTokens).toBe(CONTEXT_HARD_MAX_TOKENS);
    const tiny = compileContext([longEntity(1)], query({ maxTokens: 0 }));
    expect(tiny.maxTokens).toBe(1);
  });

  it('handles non-finite budget gracefully', () => {
    const pack = compileContext([longEntity(1)], query({ maxTokens: NaN }));
    expect(pack.maxTokens).toBe(CONTEXT_STANDARD_TOKENS);
    expect(pack.items).toHaveLength(1);
    expect(pack.tokenEstimate).toBeLessThanOrEqual(CONTEXT_STANDARD_TOKENS);
  });

  it('counts header, conflicts and superseded report toward the budget', () => {
    const oldAnswer = entityData(dataFor({ questionId: 'pref_boundary', value: 'fast' }), {
      updated_at: '2026-06-01T00:00:00Z',
    });
    const newAnswer = entityData(dataFor({ questionId: 'pref_boundary', value: 'slow' }), {
      updated_at: '2026-07-01T00:00:00Z',
    });
    const others = Array.from({ length: 10 }, (_, i) =>
      entityData(dataFor({ claim: `Padding ${'pad '.repeat(30)}${i}`, questionId: `pref_extra_${i}` })),
    );
    const pack = compileContext([oldAnswer, newAnswer, ...others], query({ maxTokens: 250 }));
    expect(pack.conflicts).toHaveLength(1);
    expect(pack.supersededIds).toContain(oldAnswer.id);
    expect(pack.serialized).toContain('CONFLICTS:');
    expect(pack.tokenEstimate).toBeLessThanOrEqual(pack.maxTokens);
  });

  it('empty pack stays empty but still serialized', () => {
    const pack = compileContext([], query({}));
    expect(pack.items).toHaveLength(0);
    expect(pack.serialized).toContain('SOUL CONTEXT');
    expect(pack.tokenEstimate).toBeLessThanOrEqual(30);
  });
});

describe('determinism and stability', () => {
  it('same state and query produce byte-identical packs', () => {
    const entities = [
      entity({ entity_type: 'boundary' }),
      entity({ entity_type: 'preference' }),
      entity({ entity_type: 'fact' }),
    ];
    const first = compileContext(entities, query({ text: 'test claim' }));
    const second = compileContext(entities, query({ text: 'test claim' }));
    expect(second.serialized).toBe(first.serialized);
    expect(second.stateVersion).toBe(first.stateVersion);
    expect(second.items.map((i) => i.id)).toEqual(first.items.map((i) => i.id));
  });

  it('state version changes when content changes', () => {
    const base = [entity({ entity_type: 'preference' })];
    const a = compileContext(base, query({}));
    const changed = entityData(dataFor({ claim: 'Totally different claim text' }));
    const b = compileContext([changed], query({}));
    expect(b.stateVersion).not.toBe(a.stateVersion);
  });

  it('same state in any input order produces the same pack', () => {
    const oldA = entityData(dataFor({ questionId: 'pref_order', value: 'x', claim: 'First claim' }), {
      updated_at: '2026-06-01T00:00:00Z',
    });
    const newA = entityData(dataFor({ questionId: 'pref_order', value: 'y', claim: 'Second claim' }), {
      updated_at: '2026-07-01T00:00:00Z',
    });
    const extra = entity({ entity_type: 'boundary' });
    const forward = [oldA, newA, extra];
    const backward = [...forward].reverse();
    const a = compileContext(forward, query({}));
    const b = compileContext(backward, query({}));
    expect(b.serialized).toBe(a.serialized);
    expect(b.conflicts).toEqual(a.conflicts);
    expect(b.supersededIds).toEqual(a.supersededIds);
    expect(b.stateVersion).toBe(a.stateVersion);
  });

  it('repeated compilation is far under the 75ms p95 budget', () => {
    const entities = Array.from({ length: 300 }, (_, i) =>
      entityData(
        dataFor({
          claim: `Memory item ${i} about topic ${i % 25} with stable wording`,
          questionId: `mem_${i}`,
          updated_at: `2026-07-${String((i % 28) + 1).padStart(2, '0')}T12:00:00Z`,
        }),
      ),
    );
    const q = query({ text: 'topic 7', maxTokens: 900 });
    const start = performance.now();
    const iterations = 50;
    for (let i = 0; i < iterations; i++) {
      compileContext(entities, q);
    }
    const elapsedMs = (performance.now() - start) / iterations;
    expect(elapsedMs).toBeLessThan(75);
    const pack = compileContext(entities, q);
    expect(pack.items.length).toBeGreaterThan(0);
  });
});

describe('cross-language golden layout (mirrors src-tauri/src/context.rs)', () => {
  it('serializes the same fixture byte-for-byte as the Rust port', () => {
    // Та же фикстура, что и в golden_serialization_matches_ts_layout в Rust.
    const dataFor2 = (over: Record<string, unknown>): string =>
      JSON.stringify({
        claim: 'Q — placeholder',
        evidence: 'stated',
        questionId: 'pref_speed',
        value: 'x',
        confidence: 0.9,
        sensitivity: 'internal',
        scope: { domains: ['preferences'], projects: [], people: [], channels: [] },
        ...over,
      });
    const entA = entityData(
      dataFor2({ claim: 'Q — concise', questionId: 'pref_speed', value: 'concise', confidence: 0.9 }),
      { id: 'ent_a', created_at: '2026-07-01T00:00:00Z', updated_at: '2026-07-10T00:00:00Z' },
    );
    const entB = entityData(
      dataFor2({ claim: 'Q — detailed', questionId: 'pref_speed', value: 'detailed', confidence: 0.8 }),
      { id: 'ent_b', created_at: '2026-05-01T00:00:00Z', updated_at: '2026-06-01T00:00:00Z' },
    );
    const entC = entityData(
      dataFor2({ claim: 'Q — never', questionId: 'bound_health', value: 'never', confidence: 0.8 }),
      { id: 'ent_c', entity_type: 'boundary', created_at: '2026-06-15T00:00:00Z', updated_at: '2026-07-05T00:00:00Z' },
    );

    const pack = compileContext([entA, entB, entC], query({}));
    expect(pack.items.map((i) => i.id)).toEqual(['ent_c', 'ent_a']);
    expect(pack.conflicts).toHaveLength(1);
    expect(pack.supersededIds).toEqual(['ent_b']);

    // Золотой литерал: жёстко совпадает с ожиданием Rust-порта.
    const expected =
      'SOUL CONTEXT\n' +
      'policy: soul-context-policy/1\n' +
      'state: 5b38f537\n' +
      `tokens: ${estimateTokens(
        'SOUL CONTEXT\npolicy: soul-context-policy/1\nstate: 5b38f537\ntokens: X of 900\nentities: 2\n[ent_c] boundary / active / internal\nQ — never\nevidence: stated\n[ent_a] preference / active / internal\nQ — concise\nevidence: stated\nCONFLICTS:\n- ent_a vs ent_b: Same calibration question (pref_speed) with different answers\nSUPERSEDED: ent_b',
      ).toLocaleString('en-US')} of 900\n` +
      'entities: 2\n' +
      '[ent_c] boundary / active / internal\n' +
      'Q — never\n' +
      'evidence: stated\n' +
      '[ent_a] preference / active / internal\n' +
      'Q — concise\n' +
      'evidence: stated\n' +
      'CONFLICTS:\n' +
      '- ent_a vs ent_b: Same calibration question (pref_speed) with different answers\n' +
      'SUPERSEDED: ent_b';
    expect(pack.serialized).toBe(expected);
    expect(pack.stateVersion).toBe('5b38f537');
    // Оценка применяется к реальному сериализованному пакету («110» на 2
    // символа длиннее «X») — этот сдвиг зафиксирован в обоих языках.
    expect(pack.tokenEstimate).toBe(110);
    expect(pack.tokenEstimate).toBe(estimateTokens(expected));
  });
});

describe('helpers', () => {

  it('collectDomains gathers unique sorted domains', () => {
    const a = entityData(dataFor({ scope: { domains: ['goals'], projects: [], people: [], channels: [] } }));
    const b = entityData(dataFor({ scope: { domains: ['preferences'], projects: [], people: [], channels: [] } }));
    const c = entityData(dataFor({ scope: { domains: ['goals'], projects: [], people: [], channels: [] } }));
    expect(collectDomains([a, b, c])).toEqual(['goals', 'preferences']);
  });

  it('default query allows internal sensitivity and active status', () => {
    const pack = compileContext([entity({})], query({}));
    expect(pack.items).toHaveLength(1);
  });
});
