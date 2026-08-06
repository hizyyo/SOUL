export interface MutationLease {
  key: string;
  release: () => void;
}

export interface MutationCoordinator {
  getSnapshot: () => string | null;
  subscribe: (listener: () => void) => () => void;
  tryAcquire: (key: string) => MutationLease | null;
}

export function createMutationCoordinator(): MutationCoordinator {
  let activeKey: string | null = null;
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of listeners) listener();
  };

  return {
    getSnapshot: () => activeKey,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    tryAcquire: (key) => {
      if (activeKey !== null) return null;
      activeKey = key;
      notify();
      let released = false;
      return {
        key,
        release: () => {
          if (released) return;
          released = true;
          if (activeKey === key) {
            activeKey = null;
            notify();
          }
        },
      };
    },
  };
}

export interface LatestRequest {
  isCurrent: () => boolean;
}

export function createLatestRequestGate(): { begin: () => LatestRequest } {
  let latest = 0;
  return {
    begin: () => {
      const request = ++latest;
      return { isCurrent: () => request === latest };
    },
  };
}

export const appMutationCoordinator = createMutationCoordinator();
