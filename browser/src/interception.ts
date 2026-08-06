interface ClosestCapable {
  closest(selector: string): unknown;
}

export function closestMatchingTarget(target: unknown, selector: string): unknown | null {
  if (!target || typeof target !== 'object' || !('closest' in target)) return null;
  const closest = (target as Partial<ClosestCapable>).closest;
  if (typeof closest !== 'function') return null;
  return closest.call(target, selector) ?? null;
}

export function isEventInside(target: unknown, selector: string): boolean {
  return closestMatchingTarget(target, selector) !== null;
}
