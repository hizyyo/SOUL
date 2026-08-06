/**
 * Мосты между абстракцией Page/PageElement и реальным DOM.
 * Реальную работу выполняют только эти функции; вся логика адаптеров
 * живёт на абстракции и тестируется без браузера.
 */

import type { Page, PageElement } from './adapters/types';

function mapElement(element: Element | null): PageElement | null {
  if (!element) {
    return null;
  }
  return {
    tagName: element.tagName,
    ariaLabel: element.getAttribute('aria-label'),
    value: element instanceof HTMLTextAreaElement ? element.value : (element.textContent ?? ''),
    connected: element.isConnected,
    editable:
      element instanceof HTMLTextAreaElement
        ? !element.disabled && !element.readOnly
        : element instanceof HTMLElement && element.isContentEditable,
    enabled:
      !(element instanceof HTMLButtonElement) ||
      (!element.disabled && element.getAttribute('aria-disabled') !== 'true'),
    click() {
      (element as HTMLElement).click();
    },
    focus() {
      (element as HTMLElement).focus();
    },
  };
}

export function createPageFromDocument(documentLike: Document, origin: string, url: string): Page {
  return {
    origin,
    url,
    querySelector(selector: string): PageElement | null {
      return mapElement(documentLike.querySelector(selector));
    },
    querySelectorAll(selector: string): PageElement[] {
      const out: PageElement[] = [];
      documentLike.querySelectorAll(selector).forEach((element) => {
        const mapped = mapElement(element);
        if (mapped) {
          out.push(mapped);
        }
      });
      return out;
    },
  };
}

/**
 * Записывает текст в textarea через нативный сеттер и поднимает input/change
 * события — так React-приложения (ChatGPT) обновляют своё состояние.
 */
export function setTextareaValue(element: HTMLTextAreaElement, text: string): boolean {
  const original = element.value;
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
  const write = (value: string): void => {
    if (setter) {
      setter.call(element, value);
    } else {
      element.value = value;
    }
  };
  return setAndVerifyText(
    () => element.value,
    write,
    () => {
      element.dispatchEvent(new Event('input', { bubbles: true }));
      element.dispatchEvent(new Event('change', { bubbles: true }));
    },
    original,
    text,
  );
}

/**
 * Вставляет текст в contenteditable (Gemini, Claude — ProseMirror/Quill):
 * фокус, выделение всего содержимого и execCommand('insertText') —
 * единственный надёжный путь обновить внутреннее состояние редактора.
 */
export function setContenteditableValue(element: HTMLElement, text: string): boolean {
  const original = element.textContent ?? '';
  const write = (value: string): void => {
    element.focus();
    const selection = window.getSelection();
    if (selection) {
      const range = document.createRange();
      range.selectNodeContents(element);
      selection.removeAllRanges();
      selection.addRange(range);
    }
    if (!document.execCommand('insertText', false, value)) {
      element.textContent = value;
    }
  };
  return setAndVerifyText(
    () => element.textContent ?? '',
    write,
    () => element.dispatchEvent(new Event('input', { bubbles: true })),
    original,
    text,
  );
}

export function setAndVerifyText(
  read: () => string,
  write: (value: string) => void,
  notify: () => void,
  original: string,
  next: string,
): boolean {
  try {
    write(next);
    notify();
    if (read() === next) {
      return true;
    }
  } catch {
    // Restore below so a failed framework mutation remains fail-open.
  }
  try {
    write(original);
    notify();
  } catch {
    // Best effort: the caller still must not auto-send after a failed mutation.
  }
  return false;
}
