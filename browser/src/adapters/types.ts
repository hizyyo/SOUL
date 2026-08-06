/**
 * Контракт site adapter-а: версионированное описание разметки одного
 * веб-чата. Абстракция Page/PageElement позволяет тестировать адаптеры
 * без реального DOM (фиктивные страницы в тестах).
 */

export interface AdapterEvent {
  type: 'keydown' | 'click';
  key?: string;
  shiftKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  isComposing?: boolean;
  targetTagName?: string;
  targetAriaLabel?: string | null;
}

export interface PageElement {
  readonly tagName: string;
  readonly ariaLabel: string | null;
  readonly value: string;
  readonly connected: boolean;
  readonly editable: boolean;
  readonly enabled: boolean;
  click(): void;
  focus(): void;
}

export interface Page {
  readonly origin: string;
  readonly url: string;
  querySelector(selector: string): PageElement | null;
  querySelectorAll(selector: string): PageElement[];
}

export interface HealthReport {
  status: 'ok' | 'failed';
  /** Обязательные селекторы, которые не найдены (пусто при ok). */
  missing: string[];
  checked: string[];
  at: number;
}

export interface SiteAdapter {
  readonly id: string;
  /** Версия разметки, под которую написан адаптер. */
  readonly version: string;
  readonly label: string;
  readonly origin: string;
  /** Селекторы, без которых адаптер считается сломанным (fail-closed). */
  readonly requiredSelectors: readonly string[];
  readonly inputSelector: string;
  readonly sendSelector: string;
  /** Элемент, после которого размещается чип SOUL. */
  readonly mountSelector: string;
  /** Контейнер истории сообщений для сворачивания блока. */
  readonly historySelector: string;
  /** Точный контейнер сообщения пользователя внутри истории. */
  readonly userMessageSelector: string;
  readonly inputKind: 'textarea' | 'contenteditable';

  /** Проверка разметки страницы; ok — только если найдено всё необходимое. */
  probe(page: Page): HealthReport;

  /** Является ли событие клавиатуры/клика отправкой сообщения. */
  isSendEvent(event: AdapterEvent): boolean;
}
