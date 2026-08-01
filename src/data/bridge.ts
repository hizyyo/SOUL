/**
 * Модель состояния Browser Companion (Native Messaging host) для настроек.
 * Поля зеркалируют src-tauri/src/native_host.rs (BridgeStatus).
 */

export interface BridgeStatus {
  host_name: string;
  registered: boolean;
  manifest_path: string;
  manifest_exists: boolean;
  binary_path: string;
  binary_exists: boolean;
  extension_id: string;
  error: string;
}

export const COMPANION_SITES = ['ChatGPT Web', 'Gemini Web', 'Claude Web'];

/** Короткий статус для UI. */
export function bridgeStateLabel(status: BridgeStatus | null): string {
  if (!status) {
    return 'Не проверено';
  }
  if (status.error) {
    return `Ошибка: ${status.error}`;
  }
  if (!status.manifest_exists) {
    return 'Host не зарегистрирован';
  }
  if (!status.binary_exists) {
    return 'Двоичный файл host-а не найден';
  }
  return status.registered ? 'Зарегистрирован' : 'Манифест есть, ключи реестра отсутствуют';
}

/** Развёрнутая заметка для UI. */
export function bridgeStatusNote(status: BridgeStatus | null): string {
  if (!status) {
    return 'Нажмите «Проверить», чтобы определить состояние Browser Companion.';
  }
  const parts = [
    `Расширение: ${status.extension_id}`,
    status.manifest_exists ? 'манифест: есть' : 'манифест: нет',
    status.registered ? 'регистрация: есть' : 'регистрация: нет',
  ];
  return parts.join(' · ');
}
