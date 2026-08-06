import { describe, expect, it } from 'vitest';
import { shouldCloseModalForKey } from '../src/components/Modal';

describe('Modal Escape policy', () => {
  it('closes on Escape by default', () => {
    expect(shouldCloseModalForKey('Escape', true)).toBe(true);
  });

  it('does not close a busy destructive dialog on Escape', () => {
    expect(shouldCloseModalForKey('Escape', false)).toBe(false);
    expect(shouldCloseModalForKey('Enter', true)).toBe(false);
  });
});
