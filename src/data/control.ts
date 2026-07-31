export type ControlState =
  'no-soul' | 'start-calibration' | 'continue-calibration' | 'review-activate' | 'active';

export interface ActionItem {
  id: string;
  label: string;
  /** 'calibration' открывает калибровку (не таба), остальное — табы. */
  target: 'calibration' | 'home' | 'inbox' | 'preview' | 'tests' | 'context' | 'settings';
  disabled: boolean;
  note: string;
}

export interface ControlCenter {
  state: ControlState;
  statusLabel: string;
  next: ActionItem;
  secondary: ActionItem[];
}

/**
 * Детерминированная модель контрольного центра: состояние SOUL, один доминирующий
 * CTA и вторичные действия. Без сети, без модели, без случайности — тестируется.
 */
export function buildControlCenter(input: {
  hasSoul: boolean;
  activated: boolean;
  calibrationStep: number;
  totalSteps: number;
  previewConfirmed: boolean;
  candidateCount: number;
}): ControlCenter {
  if (!input.hasSoul) {
    return {
      state: 'no-soul',
      statusLabel: 'Setup',
      next: {
        id: 'create-soul',
        label: 'Create SOUL',
        target: 'home',
        disabled: false,
        note: 'Start with a local SOUL — no account, no imports.',
      },
      secondary: [],
    };
  }

  const step = input.calibrationStep;
  const total = input.totalSteps;
  const calibrationDone = step >= total;

  let state: ControlState;
  let next: ActionItem;
  if (!input.activated && !calibrationDone && step <= 0) {
    state = 'start-calibration';
    next = {
      id: 'start-calibration',
      label: 'Start Calibration (5 min)',
      target: 'calibration',
      disabled: false,
      note: 'Build a useful local SOUL — no account, works offline.',
    };
  } else if (!input.activated && !calibrationDone) {
    state = 'continue-calibration';
    next = {
      id: 'continue-calibration',
      label: `Continue Calibration (step ${Math.min(step + 1, total)} of ${total})`,
      target: 'calibration',
      disabled: false,
      note: 'Your progress is saved on this device.',
    };
  } else if (!input.activated) {
    state = 'review-activate';
    next = {
      id: 'review-activate',
      label: input.previewConfirmed ? 'Activate SOUL' : 'Review & Activate',
      target: 'preview',
      disabled: false,
      note: input.previewConfirmed
        ? 'Preview is confirmed. Activate to make your SOUL live.'
        : 'Review what SOUL learned, confirm the preview, then activate.',
    };
  } else {
    state = 'active';
    next = {
      id: 'connect-ai-client',
      label: 'Connect an AI client',
      target: 'settings',
      disabled: true,
      note: 'Coming in a later update. Your SOUL is active and local.',
    };
  }

  const secondary: ActionItem[] = [];
  if (input.candidateCount > 0) {
    secondary.push({
      id: 'review-candidates',
      label: `Review candidates (${input.candidateCount})`,
      target: 'inbox',
      disabled: false,
      note: 'Confirm, edit, reject or undo — sorted by importance and risk.',
    });
  }
  secondary.push({
    id: 'check-receipts',
    label: 'Check disclosure receipts',
    target: 'settings',
    disabled: false,
    note: 'Local receipts of deletions and disclosures. No personal content inside.',
  });
  secondary.push({
    id: 'improve-soul',
    label: 'Improve SOUL',
    target: 'settings',
    disabled: true,
    note: 'Coming in a later update.',
  });

  return { state, statusLabel: input.activated ? 'Active' : 'Setup', next, secondary };
}
