import { describe, expect, it, vi } from 'vitest';
import { createLifecycleController } from '../browser/src/lifecycle';

describe('browser SPA lifecycle', () => {
  it('reconciles delayed/remounted DOM and cleans observer plus interval once', () => {
    let observed: (() => void) | null = null;
    let intervalListener: (() => void) | null = null;
    const stopObserving = vi.fn();
    const clearInterval = vi.fn();
    const reconcile = vi.fn();
    const controller = createLifecycleController(
      reconcile,
      {
        observe(listener) {
          observed = listener;
          return stopObserving;
        },
        setInterval(listener) {
          intervalListener = listener;
          return 7;
        },
        clearInterval,
      },
      1_000,
    );

    expect(reconcile).toHaveBeenCalledTimes(1);
    (observed as (() => void) | null)?.();
    (intervalListener as (() => void) | null)?.();
    expect(reconcile).toHaveBeenCalledTimes(3);
    controller.destroy();
    controller.destroy();
    expect(stopObserving).toHaveBeenCalledOnce();
    expect(clearInterval).toHaveBeenCalledWith(7);
    controller.reconcile();
    expect(reconcile).toHaveBeenCalledTimes(3);
  });
});
