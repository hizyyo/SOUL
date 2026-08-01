/**
 * Общие константы протокола SOUL Browser Companion.
 * Зеркалируют src-tauri/src/bridge.rs — при расхождении запросы отклоняются
 * host-ом (fail-closed), поэтому константы тестируются с обеих сторон.
 */

/** Фиксированный ID расширения: выведен из ключа в manifest.source.json. */
export const EXTENSION_ID = 'epfbcmgajbpjbphepfbhcoibmoaflbld';

/** Имя зарегистрированного Native Messaging host. */
export const HOST_NAME = 'com.soul.browser_companion';

/** Версия протокола host-а. */
export const PROTOCOL_VERSION = 'soul-bridge/1';

/** Поддерживаемые происхождения (и host, и контент-скрипт). */
export const SUPPORTED_ORIGINS: readonly string[] = [
  'https://chatgpt.com',
  'https://gemini.google.com',
  'https://claude.ai',
];

/** Максимальный размер native-сообщения (1 МБ, лимит Chrome). */
export const MAX_FRAME_BYTES = 1024 * 1024;

/** Максимальная длина текста задачи из поля ввода. */
export const MAX_TASK_CHARS = 8000;

/** Бюджет токенов контекста (совпадает с компилятором). */
export const DEFAULT_TOKENS = 900;
export const MAX_TOKENS = 3000;

/** Таймаут ожидания ответа host-а, мс. */
export const HOST_TIMEOUT_MS = 10_000;
