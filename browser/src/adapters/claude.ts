/**
 * Адаптер Claude Web (claude.ai), разметка v1.
 */

import type { AdapterEvent, HealthReport, Page, SiteAdapter } from './types';

export const claudeAdapter: SiteAdapter = {
  id: 'claude',
  version: '1',
  label: 'Claude Web',
  origin: 'https://claude.ai',

  requiredSelectors: ['div[contenteditable="true"].ProseMirror'],
  inputSelector: 'div[contenteditable="true"].ProseMirror',
  sendSelector: 'button[aria-label="Send message"]',
  mountSelector: 'form[data-testid="chat-input-form"]',
  historySelector: 'div[data-testid="chat-history"]',
  inputKind: 'contenteditable',

  probe(page: Page): HealthReport {
    const input = page.querySelector('div[contenteditable="true"].ProseMirror');
    const send = page.querySelector('button[aria-label="Send message"]');
    const mount = page.querySelector('form[data-testid="chat-input-form"]');
    const missing = [
      ...(input ? [] : ['div[contenteditable="true"].ProseMirror']),
      ...(send ? [] : ['button[aria-label="Send message"]']),
      ...(mount ? [] : ['form[data-testid="chat-input-form"]']),
    ];
    return {
      status: missing.length === 0 ? 'ok' : 'failed',
      missing,
      checked: [
        'div[contenteditable="true"].ProseMirror',
        'button[aria-label="Send message"]',
        'form[data-testid="chat-input-form"]',
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
