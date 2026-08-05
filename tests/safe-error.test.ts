import { describe, expect, it } from 'vitest';
import { safeErrorMessage } from '../src/data/safeError';

describe('safeErrorMessage', () => {
  it('shows a user-safe action and correlation ID without backend details', () => {
    const message = safeErrorMessage('загрузить локальные данные', 'S16-test-1');
    expect(message).toContain('загрузить локальные данные');
    expect(message).toContain('S16-test-1');
    expect(message).not.toContain('C:\\Users');
  });
});
