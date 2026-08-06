import { MAX_TASK_CHARS } from './constants';

export interface SubmissionSnapshot {
  readonly route: string;
  readonly draft: string;
  readonly contextQuery: string;
}

export interface SubmissionCandidate {
  readonly route: string;
  readonly draft: string;
  readonly connected: boolean;
}

export function createSubmissionSnapshot(
  route: string,
  draft: string,
): SubmissionSnapshot | null {
  if (draft.trim() === '') {
    return null;
  }
  return {
    route,
    draft,
    contextQuery: Array.from(draft).slice(0, MAX_TASK_CHARS).join(''),
  };
}

export function submissionStillMatches(
  snapshot: SubmissionSnapshot,
  candidate: SubmissionCandidate,
): boolean {
  return (
    candidate.connected &&
    candidate.route === snapshot.route &&
    candidate.draft === snapshot.draft
  );
}
