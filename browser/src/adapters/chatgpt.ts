/**
 * Адаптер ChatGPT Web (chatgpt.com), разметка v1.
 */

import { probeAdapter } from './probe';
import type { AdapterEvent, SiteAdapter } from './types';

export const chatgptAdapter: SiteAdapter = {
  id: 'chatgpt',
  version: '1',
  label: 'ChatGPT Web',
  origin: 'https://chatgpt.com',

  requiredSelectors: ['#prompt-textarea'],
  inputSelector: '#prompt-textarea',
  sendSelector: 'button[data-testid="send-button"]',
  mountSelector: 'form[data-type="unified-composer"]',
  historySelector: 'main',
  userMessageSelector: '[data-message-author-role="user"]',
  inputKind: 'textarea',

  probe(page) {
    return probeAdapter(this, page, { inputTag: 'TEXTAREA', mountTag: 'FORM' });
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
