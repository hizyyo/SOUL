import { describe, expect, it, vi } from 'vitest';
import { createFrontendDeviceId, resolveDeviceId } from '../src/data/identity';

describe('frontend device identity', () => {
  it('creates device ids from crypto.randomUUID-compatible values', () => {
    expect(createFrontendDeviceId(() => '123e4567-e89b-12d3-a456-426614174000')).toBe(
      'device_123e4567-e89b-12d3-a456-426614174000',
    );
  });

  it('persists a new UUID-backed id and reuses a stored id', () => {
    const values = new Map<string, string>();
    const storage = {
      getItem: vi.fn((key: string) => values.get(key) ?? null),
      setItem: vi.fn((key: string, value: string) => values.set(key, value)),
    };
    const randomUUID = vi.fn(() => '123e4567-e89b-12d3-a456-426614174000');

    expect(resolveDeviceId(storage, randomUUID)).toBe(
      'device_123e4567-e89b-12d3-a456-426614174000',
    );
    expect(resolveDeviceId(storage, randomUUID)).toBe(
      'device_123e4567-e89b-12d3-a456-426614174000',
    );
    expect(randomUUID).toHaveBeenCalledTimes(1);
    expect(storage.setItem).toHaveBeenCalledTimes(1);
  });
});
