import { describe, expect, it } from 'vitest';
import { tabIndexForKey } from '../src/data/nav';

describe('tabIndexForKey', () => {
  it('wraps with arrow keys and supports Home/End', () => {
    expect(tabIndexForKey(0, 4, 'ArrowLeft')).toBe(3);
    expect(tabIndexForKey(3, 4, 'ArrowRight')).toBe(0);
    expect(tabIndexForKey(2, 4, 'Home')).toBe(0);
    expect(tabIndexForKey(1, 4, 'End')).toBe(3);
  });

  it('ignores unrelated keys', () => {
    expect(tabIndexForKey(1, 4, 'Tab')).toBeNull();
  });
});
