import { describe, it, expect } from 'vitest';
import type { CalibrationQuestion } from '../src/data/calibration';
import {
  buildEntityData,
  computeActivationScore,
  detectSensitivity,
  maskText,
  parseEntityData,
  rankCandidates,
  type ReviewEntity,
} from '../src/data/review';

const BINARY_Q: CalibrationQuestion = {
  id: 'pref_1',
  type: 'binary',
  category: 'preference',
  prompt: 'Do you prefer concise answers or detailed explanations?',
  options: ['Concise', 'Detailed'],
  optional: false,
};

const TEXT_Q: CalibrationQuestion = {
  id: 'text_1',
  type: 'text',
  category: 'fact',
  prompt: 'Describe your work or main project in a few sentences.',
  optional: false,
};

const BOUND_Q: CalibrationQuestion = {
  id: 'bound_1',
  type: 'multiple',
  category: 'boundary',
  prompt: 'What topics do you never want AI to decide without you?',
  options: ['Financial decisions', 'Legal commitments'],
  optional: false,
};

function entity(over: Partial<ReviewEntity>): ReviewEntity {
  return {
    id: 'ent_x',
    entity_type: 'preference',
    status: 'candidate',
    data: '{}',
    created_at: '2026-07-31T10:00:00Z',
    ...over,
  };
}

describe('buildEntityData', () => {
  it('builds enriched data for a binary answer', () => {
    const data = buildEntityData(BINARY_Q, { questionId: 'pref_1', value: 'Concise' });
    expect(data).not.toBeNull();
    expect(data!.claim).toBe('Do you prefer concise answers or detailed explanations? — Concise');
    expect(data!.evidence).toBe(BINARY_Q.prompt);
    expect(data!.confidence).toBe(0.9);
    expect(data!.explicitness).toBe(1);
    expect(data!.kind).toBe('binary');
    expect(data!.scope.domains).toEqual(['preferences']);
    expect(data!.risk).toBe(false);
    expect(data!.sensitivity).toBe('internal');
  });

  it('returns null for empty text answers', () => {
    expect(buildEntityData(TEXT_Q, { questionId: 'text_1', value: '   ' })).toBeNull();
  });

  it('keeps plain text as claim for text answers', () => {
    const data = buildEntityData(TEXT_Q, { questionId: 'text_1', value: 'I build desktop apps.' });
    expect(data!.claim).toBe('I build desktop apps.');
    expect(data!.kind).toBe('text');
    expect(data!.confidence).toBe(0.7);
    expect(data!.scope.domains).toEqual(['personal']);
  });

  it('marks boundaries as risk-bearing', () => {
    const data = buildEntityData(BOUND_Q, { questionId: 'bound_1', value: 'Financial decisions' });
    expect(data!.risk).toBe(true);
    expect(data!.scope.domains).toEqual(['boundaries']);
    expect(data!.sensitivity).toBe('private');
  });

  it('handles XSS-like text as a plain string', () => {
    const data = buildEntityData(TEXT_Q, {
      questionId: 'text_1',
      value: '<img src=x onerror=alert(1)>',
    });
    expect(data!.claim).toBe('<img src=x onerror=alert(1)>');
  });
});

describe('maskText', () => {
  it('masks email addresses', () => {
    expect(maskText('Contact me at ilya@example.com please.')).toContain('[email]');
    expect(maskText('Contact me at ilya@example.com please.')).not.toContain('ilya@example.com');
  });

  it('masks email with Cyrillic local part and domain', () => {
    expect(maskText('Пишите на почта@яндекс.рф пожалуйста')).toContain('[email]');
    expect(maskText('Пишите на почта@яндекс.рф пожалуйста')).not.toContain('почта@яндекс.рф');
  });

  it('masks API keys and bearer tokens', () => {
    expect(maskText('key sk-abcDEF1234567890xyz')).toContain('[key]');
    expect(maskText('Authorization: Bearer abc.def.ghi1234567890')).toContain('[token]');
  });

  it('masks long numeric sequences', () => {
    expect(maskText('card 4111111111111111')).toContain('[number]');
  });

  it('masks 16-digit numbers with separators', () => {
    expect(maskText('card 4111 1111 1111 1111')).toContain('[number]');
    expect(maskText('card 4111-1111-1111-1111')).toContain('[number]');
  });

  it('does not mask short numeric groups', () => {
    expect(maskText('pin 1234, code 123456')).toBe('pin 1234, code 123456');
  });

  it('does not mask ISO datetimes', () => {
    expect(maskText('created 2026-07-31T21:04:08Z')).toContain('2026-07-31T21:04:08Z');
  });

  it('masks phone-like strings', () => {
    expect(maskText('call +7 900 123 45 67 now')).toContain('[phone]');
    expect(maskText('call 900-123-4567 now')).toContain('[phone]');
  });

  it('does not mask dates', () => {
    expect(maskText('Signed on 2026-07-31.')).toContain('2026-07-31');
  });

  it('masks secret assignments', () => {
    expect(maskText('password=hunter2')).toContain('[secret]');
    expect(maskText('Пароль: hunter2')).toContain('[secret]');
  });

  it('leaves normal prose unchanged', () => {
    const prose = 'I prefer concise technical answers without emojis.';
    expect(maskText(prose)).toBe(prose);
  });

  it('handles empty input', () => {
    expect(maskText('')).toBe('');
  });
});

describe('detectSensitivity', () => {
  it('flags secret-like content as sensitive', () => {
    expect(detectSensitivity('My password is hunter2', 'fact')).toBe('sensitive');
    expect(detectSensitivity('api_key=abc123', 'fact')).toBe('sensitive');
  });

  it('flags personal contact details as private', () => {
    expect(detectSensitivity('Write to me at a@b.co', 'fact')).toBe('private');
    expect(detectSensitivity('Пишите на почта@яндекс.рф', 'fact')).toBe('private');
  });

  it('keeps boundaries at least private', () => {
    expect(detectSensitivity('Never decide finances for me', 'boundary')).toBe('private');
  });

  it('returns internal for plain content', () => {
    expect(detectSensitivity('I build desktop apps', 'preference')).toBe('internal');
  });
});

describe('computeActivationScore', () => {
  it('rewards explicitness', () => {
    const explicit = computeActivationScore({ explicitness: 1, sensitivity: 'internal' });
    const vague = computeActivationScore({ explicitness: 0.5, sensitivity: 'internal' });
    expect(explicit).toBeGreaterThan(vague);
  });

  it('penalizes sensitivity', () => {
    const internal = computeActivationScore({ explicitness: 1, sensitivity: 'internal' });
    const sensitive = computeActivationScore({ explicitness: 1, sensitivity: 'sensitive' });
    expect(internal).toBeGreaterThan(sensitive);
  });

  it('handles missing metadata', () => {
    expect(Number.isFinite(computeActivationScore({}))).toBe(true);
  });
});

describe('rankCandidates', () => {
  const boundary = entity({
    id: 'b1',
    entity_type: 'boundary',
    data: JSON.stringify({
      claim: 'Never decide finances',
      sensitivity: 'private',
      explicitness: 1,
    }),
  });
  const sensitive = entity({
    id: 's1',
    entity_type: 'fact',
    data: JSON.stringify({ claim: 'My password is x', sensitivity: 'sensitive', explicitness: 1 }),
  });
  const highScore = entity({
    id: 'h1',
    entity_type: 'preference',
    data: JSON.stringify({ claim: 'Concise', sensitivity: 'internal', explicitness: 1 }),
    created_at: '2026-07-30T10:00:00Z',
  });
  const lowScore = entity({
    id: 'l1',
    entity_type: 'preference',
    data: JSON.stringify({ claim: 'Maybe concise', sensitivity: 'internal', explicitness: 0.3 }),
    created_at: '2026-07-29T10:00:00Z',
  });
  const legacy = entity({
    id: 'l2',
    entity_type: 'preference',
    data: JSON.stringify({ claim: 'Old format entity' }),
  });

  it('puts risk-bearing candidates first', () => {
    const sorted = rankCandidates([highScore, boundary, sensitive, lowScore, legacy]);
    expect(sorted[0]!.id).toBe('b1');
    expect(sorted[1]!.id).toBe('s1');
  });

  it('sorts remaining by score then recency', () => {
    const sorted = rankCandidates([lowScore, highScore, legacy]);
    expect(sorted[0]!.id).toBe('h1');
    expect(sorted[1]!.id).toBe('l2');
    expect(sorted[2]!.id).toBe('l1');
  });

  it('does not mutate the input array', () => {
    const input = [lowScore, highScore];
    rankCandidates(input);
    expect(input[0]!.id).toBe('l1');
  });
});

describe('parseEntityData', () => {
  it('parses valid JSON data', () => {
    const data = parseEntityData('{"claim":"x","confidence":0.9}');
    expect(data.claim).toBe('x');
    expect(data.confidence).toBe(0.9);
  });

  it('returns empty object for invalid data', () => {
    expect(parseEntityData('not json')).toEqual({});
    expect(parseEntityData('[]')).toEqual({});
  });
});
