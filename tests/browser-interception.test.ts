import { describe, expect, it, vi } from 'vitest';
import { closestMatchingTarget, isEventInside } from '../browser/src/interception';

describe('browser event interception scope', () => {
  it('intercepts key events only when the target is inside the composer', () => {
    const composer = {};
    const target = { closest: vi.fn((selector: string) => (selector === '#composer' ? composer : null)) };
    expect(isEventInside(target, '#composer')).toBe(true);
    expect(isEventInside(target, '#other')).toBe(false);
  });

  it('finds a delegated send button through a nested SPA click target', () => {
    const sendButton = { tagName: 'BUTTON' };
    const icon = { closest: vi.fn(() => sendButton) };
    expect(closestMatchingTarget(icon, 'button[data-send]')).toBe(sendButton);
  });

  it('ignores targets that cannot participate in closest matching', () => {
    expect(closestMatchingTarget(null, '#composer')).toBeNull();
    expect(closestMatchingTarget({}, '#composer')).toBeNull();
  });
});
