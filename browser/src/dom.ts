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
export function setTextareaValue(element: HTMLTextAreaElement, text: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')?.set;
  if (setter) {
    setter.call(element, text);
  } else {
    element.value = text;
  }
  element.dispatchEvent(new Event('input', { bubbles: true }));
  element.dispatchEvent(new Event('change', { bubbles: true }));
}

/**
 * Вставляет текст в contenteditable (Gemini, Claude — ProseMirror/Quill):
 * фокус, выделение всего содержимого и execCommand('insertText') —
 * единственный надёжный путь обновить внутреннее состояние редактора.
 */
export function setContenteditableValue(element: HTMLElement, text: string): void {
  element.focus();
  const selection = window.getSelection();
  if (selection) {
    const range = document.createRange();
    range.selectNodeContents(element);
    selection.removeAllRanges();
    selection.addRange(range);
  }
  document.execCommand('insertText', false, text);
}
