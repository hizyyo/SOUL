import { useSyncExternalStore } from 'react';
import { appMutationCoordinator } from './mutations';

export function useGlobalMutation(): {
  activeKey: string | null;
  run: <T>(
    key: string,
    task: () => Promise<T>,
  ) => Promise<{ started: true; value: T } | { started: false }>;
} {
  const activeKey = useSyncExternalStore(
    appMutationCoordinator.subscribe,
    appMutationCoordinator.getSnapshot,
    appMutationCoordinator.getSnapshot,
  );

  return {
    activeKey,
    run: async <T>(key: string, task: () => Promise<T>) => {
      const lease = appMutationCoordinator.tryAcquire(key);
      if (!lease) return { started: false };
      try {
        return { started: true, value: await task() };
      } finally {
        lease.release();
      }
    },
  };
}
