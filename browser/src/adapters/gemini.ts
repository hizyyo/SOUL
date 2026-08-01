/**
 * Адаптер Gemini Web (gemini.google.com), разметка v1.
 */

import type { AdapterEvent, HealthReport, Page, SiteAdapter } from './types';

export const geminiAdapter: SiteAdapter = {
  id: 'gemini',
  version: '1',
  label: 'Gemini Web',
  origin: 'https://gemini.google.com',

  requiredSelectors: ['div.ql-editor[contenteditable="true"]'],
  inputSelector: 'div.ql-editor[contenteditable="true"]',
  sendSelector: 'button[aria-label="Send message"]',
  mountSelector: 'div.ql-container',
  historySelector: 'main',
  inputKind: 'contenteditable',

  probe(page: Page): HealthReport {
    const input = page.querySelector('div.ql-editor[contenteditable="true"]');
    const send = page.querySelector('button[aria-label="Send message"]');
    const mount = page.querySelector('div.ql-container');
    const missing = [
      ...(input ? [] : ['div.ql-editor[contenteditable="true"]']),
      ...(send ? [] : ['button[aria-label="Send message"]']),
      ...(mount ? [] : ['div.ql-container']),
    ];
    return {
      status: missing.length === 0 ? 'ok' : 'failed',
      missing,
      checked: [
        'div.ql-editor[contenteditable="true"]',
        'button[aria-label="Send message"]',
        'div.ql-container',
      ],
      at: Date.now(),
    };
  },

  isSendEvent(event: AdapterEvent): boolean {
    return isEnterKeydown(event) || isSendButtonClick(event);
  },
};

function isEnterKeydown(event: AdapterEvent): boolean {
  return (
    event.type === 'keydown' &&
    event.key === 'Enter' &&
    !event.shiftKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey &&
    !event.isComposing
  );
}

function isSendButtonClick(event: AdapterEvent): boolean {
  if (event.type !== 'click') {
    return false;
  }
  const aria = event.targetAriaLabel;
  return event.targetTagName === 'BUTTON' && aria === 'Send message';
}
