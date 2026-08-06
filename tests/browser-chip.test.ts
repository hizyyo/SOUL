import { describe, expect, it } from 'vitest';
import { chipViewModel } from '../browser/src/ui/chip';

describe('browser companion chip accessibility', () => {
  it('keeps a persistent accessible primary control in all states', () => {
    expect(
      chipViewModel({ state: 'on', count: 1, oneMessageOff: false, sessionOff: false }),
    ).toMatchObject({ active: true, oneMessageVisible: true, sessionOffVisible: true });
    expect(
      chipViewModel({ state: 'off', count: null, oneMessageOff: false, sessionOff: true }),
    ).toMatchObject({ active: false, primaryAriaLabel: 'Enable SOUL' });
    expect(
      chipViewModel({
        state: 'error',
        count: null,
        oneMessageOff: false,
        sessionOff: false,
        errorHint: 'Composer unavailable',
      }).primaryAriaLabel,
    ).toContain('Composer unavailable');
  });
});
