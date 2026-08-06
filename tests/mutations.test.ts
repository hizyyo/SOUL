import { describe, expect, it, vi } from 'vitest';
import { createLatestRequestGate, createMutationCoordinator } from '../src/data/mutations';

describe('mutation coordination', () => {
  it('allows only one global mutation until its lease is released', () => {
    const coordinator = createMutationCoordinator();
    const listener = vi.fn();
    coordinator.subscribe(listener);

    const first = coordinator.tryAcquire('entity:a');
    expect(first).not.toBeNull();
    expect(coordinator.getSnapshot()).toBe('entity:a');
    expect(coordinator.tryAcquire('policy:b')).toBeNull();

    first?.release();
    expect(coordinator.getSnapshot()).toBeNull();
    expect(coordinator.tryAcquire('policy:b')).not.toBeNull();
    expect(listener).toHaveBeenCalledTimes(3);
  });

  it('marks older refreshes stale when a newer refresh starts', () => {
    const gate = createLatestRequestGate();
    const first = gate.begin();
    const second = gate.begin();
    expect(first.isCurrent()).toBe(false);
    expect(second.isCurrent()).toBe(true);
  });
});
