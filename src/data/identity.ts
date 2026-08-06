const LEGACY_DEVICE_ID_KEY = 'soul_device_id';

export function resolveDeviceId(
  storage: Pick<Storage, 'getItem' | 'setItem'>,
  randomUUID: () => string = () => crypto.randomUUID(),
): string {
  const stored = storage.getItem(LEGACY_DEVICE_ID_KEY);
  if (stored?.trim()) return stored;

  const id = createFrontendDeviceId(randomUUID);
  storage.setItem(LEGACY_DEVICE_ID_KEY, id);
  return id;
}

export function createFrontendDeviceId(
  randomUUID: () => string = () => crypto.randomUUID(),
): string {
  return `device_${randomUUID()}`;
}
