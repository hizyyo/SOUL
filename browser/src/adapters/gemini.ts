/**
 * Адаптер Gemini Web (gemini.google.com), разметка v1.
 */

import { probeAdapter } from './probe';
import type { AdapterEvent, SiteAdapter } from './types';

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
  userMessageSelector: 'user-query',
  inputKind: 'contenteditable',

  probe(page) {
    return probeAdapter(this, page, { inputTag: 'DIV', mountTag: 'DIV' });
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
