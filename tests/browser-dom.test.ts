import { describe, expect, it, vi } from 'vitest';
import { setAndVerifyText } from '../browser/src/dom';

describe('browser composer setter verification', () => {
  it('accepts a verified framework write', () => {
    let value = 'draft';
    const notify = vi.fn();
    expect(setAndVerifyText(() => value, (next) => { value = next; }, notify, value, 'composed')).toBe(true);
    expect(value).toBe('composed');
    expect(notify).toHaveBeenCalledOnce();
  });

  it('restores the original draft when the framework rejects the write', () => {
    let value = 'draft';
    const writes: string[] = [];
    expect(
      setAndVerifyText(
        () => value,
        (next) => {
          writes.push(next);
          if (next === 'draft') value = next;
        },
        () => {},
        value,
        'composed',
      ),
    ).toBe(false);
    expect(value).toBe('draft');
    expect(writes).toEqual(['composed', 'draft']);
  });
});
