/**
 * Адаптер ChatGPT Web (chatgpt.com), разметка v1.
 */

import type { AdapterEvent, HealthReport, Page, SiteAdapter } from './types';

export const chatgptAdapter: SiteAdapter = {
  id: 'chatgpt',
  version: '1',
  label: 'ChatGPT Web',
  origin: 'https://chatgpt.com',

  requiredSelectors: ['#prompt-textarea'],
  inputSelector: '#prompt-textarea',
  sendSelector: 'button[data-testid="send-button"]',
  mountSelector: 'form[data-type="unified-composer"]',
  historySelector: '[data-message-author-role]',
  inputKind: 'textarea',

  probe(page: Page): HealthReport {
    const required = page.querySelector('#prompt-textarea');
    const send = page.querySelector('button[data-testid="send-button"]');
    const mount = page.querySelector('form[data-type="unified-composer"]');
    const missing = [
      ...(required ? [] : ['#prompt-textarea']),
      ...(send ? [] : ['button[data-testid="send-button"]']),
      ...(mount ? [] : ['form[data-type="unified-composer"]']),
    ];
    return {
      status: missing.length === 0 ? 'ok' : 'failed',
      missing,
      checked: [
        '#prompt-textarea',
        'button[data-testid="send-button"]',
        'form[data-type="unified-composer"]',
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
  return event.targetTagName === 'BUTTON' && (aria === 'Send prompt' || aria === 'Send message');
}
