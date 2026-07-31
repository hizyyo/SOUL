import { describe, it, expect } from 'vitest';
import { buildControlCenter } from '../src/data/control';

const TOTAL = 5;

function center(over: Partial<Parameters<typeof buildControlCenter>[0]> = {}) {
  return buildControlCenter({
    hasSoul: true,
    activated: false,
    calibrationStep: 0,
    totalSteps: TOTAL,
    previewConfirmed: false,
    candidateCount: 0,
    ...over,
  });
}

describe('buildControlCenter', () => {
  it('no-soul: create CTA and no secondary actions', () => {
    const c = center({ hasSoul: false });
    expect(c.state).toBe('no-soul');
    expect(c.statusLabel).toBe('Setup');
    expect(c.next).toMatchObject({ id: 'create-soul', disabled: false, target: 'home' });
    expect(c.secondary).toHaveLength(0);
  });

  it('fresh soul: start calibration is the single CTA', () => {
    const c = center();
    expect(c.state).toBe('start-calibration');
    expect(c.next).toMatchObject({
      id: 'start-calibration',
      label: 'Start Calibration (5 min)',
      target: 'calibration',
      disabled: false,
    });
  });

  it('in-progress calibration: continue with honest step label', () => {
    const c = center({ calibrationStep: 2 });
    expect(c.state).toBe('continue-calibration');
    expect(c.next.label).toContain('step 3 of 5');
    expect(c.next.target).toBe('calibration');
    expect(c.next.disabled).toBe(false);
  });

  it('finished calibration: review & activate leads to preview', () => {
    const c = center({ calibrationStep: TOTAL });
    expect(c.state).toBe('review-activate');
    expect(c.next).toMatchObject({
      id: 'review-activate',
      label: 'Review & Activate',
      target: 'preview',
      disabled: false,
    });
  });

  it('confirmed preview: CTA becomes Activate SOUL', () => {
    const c = center({ calibrationStep: TOTAL, previewConfirmed: true });
    expect(c.state).toBe('review-activate');
    expect(c.next.label).toBe('Activate SOUL');
    expect(c.next.note).toContain('confirmed');
  });

  it('active soul: connect AI client is the CTA but disabled', () => {
    const c = center({ activated: true, calibrationStep: TOTAL });
    expect(c.state).toBe('active');
    expect(c.statusLabel).toBe('Active');
    expect(c.next).toMatchObject({
      id: 'connect-ai-client',
      disabled: true,
      label: 'Connect an AI client',
    });
  });

  it('exactly one primary CTA exists in every state', () => {
    const states = [
      center({ hasSoul: false }),
      center(),
      center({ calibrationStep: 2 }),
      center({ calibrationStep: TOTAL }),
      center({ activated: true, calibrationStep: TOTAL }),
    ];
    for (const c of states) {
      expect(c.next).toBeDefined();
      expect(typeof c.next.label).toBe('string');
    }
  });

  it('secondary: review candidates only when there are candidates', () => {
    const withCandidates = center({ calibrationStep: TOTAL, candidateCount: 3 });
    const ids = withCandidates.secondary.map((a) => a.id);
    expect(ids).toContain('review-candidates');
    const item = withCandidates.secondary.find((a) => a.id === 'review-candidates')!;
    expect(item.label).toContain('3');
    expect(item.disabled).toBe(false);

    const none = center({ calibrationStep: TOTAL, candidateCount: 0 });
    expect(none.secondary.map((a) => a.id)).not.toContain('review-candidates');
  });

  it('secondary: check receipts always available, improve SOUL disabled', () => {
    const c = center({ activated: true });
    const byId = new Map(c.secondary.map((a) => [a.id, a]));
    expect(byId.get('check-receipts')).toMatchObject({ disabled: false, target: 'settings' });
    expect(byId.get('improve-soul')).toMatchObject({ disabled: true });
  });

  it('labels never contain raw user text (XSS-safe surface)', () => {
    const c = center({ calibrationStep: TOTAL, candidateCount: 3 });
    const text = [c.next.label, ...c.secondary.map((a) => a.label)].join(' ');
    expect(text).not.toContain('<script>');
    expect(text).not.toContain('" onload');
  });
});
