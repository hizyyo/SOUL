export function tabIndexForKey(current: number, length: number, key: string): number | null {
  if (length === 0) return null;
  if (key === 'ArrowRight') return (current + 1) % length;
  if (key === 'ArrowLeft') return (current - 1 + length) % length;
  if (key === 'Home') return 0;
  if (key === 'End') return length - 1;
  return null;
}

export function selectedTabId<T extends string>(active: T, available: readonly T[]): T | null {
  if (available.length === 0) return null;
  return available.includes(active) ? active : (available[0] ?? null);
}

export function tabStopFor<T extends string>(tab: T, selected: T | null): 0 | -1 {
  return tab === selected ? 0 : -1;
}
