import { describe, expect, it } from 'vitest';
import { selectedTabId, tabIndexForKey, tabStopFor } from '../src/data/nav';

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

describe('tab selection', () => {
  it('always leaves exactly one available tab selected and tabbable', () => {
    const tabs = ['home', 'inbox', 'settings'] as const;
    const selected = selectedTabId('preview', tabs);
    expect(selected).toBe('home');
    expect(tabs.map((tab) => tabStopFor(tab, selected))).toEqual([0, -1, -1]);
  });

  it('keeps the active tab when it is available', () => {
    const tabs = ['home', 'preview', 'settings'] as const;
    const selected = selectedTabId('preview', tabs);
    expect(selected).toBe('preview');
    expect(tabs.filter((tab) => tabStopFor(tab, selected) === 0)).toEqual(['preview']);
  });
});
