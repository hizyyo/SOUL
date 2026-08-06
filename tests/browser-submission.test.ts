import { describe, expect, it } from 'vitest';
import { MAX_TASK_CHARS } from '../browser/src/constants';
import {
  createSubmissionSnapshot,
  submissionStillMatches,
} from '../browser/src/submission';

describe('browser submission snapshots', () => {
  it('skips blank sends without consuming or composing context', () => {
    expect(createSubmissionSnapshot('/chat', '')).toBeNull();
    expect(createSubmissionSnapshot('/chat', '  \n ')).toBeNull();
  });

  it('preserves the full draft while bounding only the lookup query', () => {
    const draft = `prefix-${'x'.repeat(MAX_TASK_CHARS)}-suffix`;
    const snapshot = createSubmissionSnapshot('/chat', draft);
    expect(snapshot?.draft).toBe(draft);
    expect(Array.from(snapshot?.contextQuery ?? '')).toHaveLength(MAX_TASK_CHARS);
  });

  it('aborts when the composer disconnects or route/draft becomes stale', () => {
    const snapshot = createSubmissionSnapshot('/chat/1', 'draft');
    expect(snapshot).not.toBeNull();
    if (!snapshot) return;
    expect(
      submissionStillMatches(snapshot, { route: '/chat/1', draft: 'draft', connected: true }),
    ).toBe(true);
    expect(
      submissionStillMatches(snapshot, { route: '/chat/2', draft: 'draft', connected: true }),
    ).toBe(false);
    expect(
      submissionStillMatches(snapshot, { route: '/chat/1', draft: 'edited', connected: true }),
    ).toBe(false);
    expect(
      submissionStillMatches(snapshot, { route: '/chat/1', draft: 'draft', connected: false }),
    ).toBe(false);
  });
});
