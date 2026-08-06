/**
 * Адаптер Claude Web (claude.ai), разметка v1.
 */

import { probeAdapter } from './probe';
import type { AdapterEvent, SiteAdapter } from './types';

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
  userMessageSelector: '[data-testid="user-message"]',
  inputKind: 'contenteditable',

  probe(page) {
    return probeAdapter(this, page, { inputTag: 'DIV', mountTag: 'FORM' });
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
