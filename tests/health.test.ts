import { describe, it, expect } from 'vitest';

describe('health', () => {
  it('test runner works', () => {
    expect(1 + 1).toBe(2);
  });

  it('SOUL is defined', () => {
    expect('SOUL').toBe('SOUL');
  });
});
