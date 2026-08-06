export interface LifecycleScheduler {
  observe(listener: () => void): () => void;
  setInterval(listener: () => void, intervalMs: number): unknown;
  clearInterval(handle: unknown): void;
}

export interface LifecycleController {
  reconcile(): void;
  destroy(): void;
}

/** Один observer + один interval; повторные сигналы не создают новые ресурсы. */
export function createLifecycleController(
  reconcile: () => void,
  scheduler: LifecycleScheduler,
  intervalMs: number,
): LifecycleController {
  let destroyed = false;
  let running = false;
  let pending = false;

  const run = (): void => {
    if (destroyed) {
      return;
    }
    if (running) {
      pending = true;
      return;
    }
    running = true;
    try {
      reconcile();
    } finally {
      running = false;
    }
    if (pending) {
      pending = false;
      run();
    }
  };

  const stopObserving = scheduler.observe(run);
  const interval = scheduler.setInterval(run, intervalMs);
  run();

  return {
    reconcile: run,
    destroy(): void {
      if (destroyed) {
        return;
      }
      destroyed = true;
      stopObserving();
      scheduler.clearInterval(interval);
    },
  };
}
