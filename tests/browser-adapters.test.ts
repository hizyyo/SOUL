import { describe, it, expect } from 'vitest';
import type { Page, PageElement } from '../browser/src/adapters/types';
import { chatgptAdapter } from '../browser/src/adapters/chatgpt';
import { geminiAdapter } from '../browser/src/adapters/gemini';
import { claudeAdapter } from '../browser/src/adapters/claude';
import { ADAPTERS, adapterForOrigin, adapterVersionId } from '../browser/src/adapters/registry';

function element(over: Partial<PageElement> = {}): PageElement {
  return {
    tagName: 'DIV',
    ariaLabel: null,
    value: '',
    connected: true,
    editable: true,
    enabled: true,
    click: () => {},
    focus: () => {},
    ...over,
  };
}

function fakePage(selectorToElement: Record<string, PageElement | null>): Page {
  return {
    origin: 'https://chatgpt.com',
    url: 'https://chatgpt.com/',
    querySelector(selector) {
      return selectorToElement[selector] ?? null;
    },
    querySelectorAll() {
      return [];
    },
  };
}

describe('registry', () => {
  it('покрывает все поддерживаемые сайты с версионированием', () => {
    expect(ADAPTERS.map((a) => a.origin).sort()).toEqual([
      'https://chatgpt.com',
      'https://claude.ai',
      'https://gemini.google.com',
    ]);
    expect(adapterVersionId(chatgptAdapter)).toBe('chatgpt/v1');
    expect(adapterForOrigin('https://gemini.google.com')).toBe(geminiAdapter);
    expect(adapterForOrigin('https://claude.ai')).toBe(claudeAdapter);
    expect(adapterForOrigin('https://evil.example')).toBeNull();
  });
});

describe('adapter probe: fail-closed при изменении разметки', () => {
  for (const adapter of ADAPTERS) {
    it(`${adapter.id}: ok при полной разметке`, () => {
      const selectorToElement: Record<string, PageElement | null> = {};
      for (const selector of adapter.requiredSelectors) {
        selectorToElement[selector] = element({
          tagName: adapter.inputKind === 'textarea' ? 'TEXTAREA' : 'DIV',
        });
      }
      selectorToElement[adapter.sendSelector] = element({ tagName: 'BUTTON' });
      selectorToElement[adapter.mountSelector] = element({
        tagName: adapter.id === 'gemini' ? 'DIV' : 'FORM',
      });
      const report = adapter.probe(fakePage(selectorToElement));
      expect(report.status).toBe('ok');
      expect(report.missing).toEqual([]);
    });

    it(`${adapter.id}: failed при отсутствии обязательных элементов`, () => {
      const report = adapter.probe(fakePage({}));
      expect(report.status).toBe('failed');
      expect(report.missing).not.toEqual([]);
      expect(report.missing).toContain(adapter.inputSelector);
      const known = [...adapter.requiredSelectors, adapter.sendSelector, adapter.mountSelector];
      expect(report.missing.every((problem) => known.some((selector) => problem.startsWith(selector)))).toBe(true);
    });

    it(`${adapter.id}: failed при изменённой разметке (устаревший селектор)`, () => {
      const stale = { [adapter.inputSelector]: element(), 'div.new-input': element() };
      const report = adapter.probe(fakePage(stale));
      expect(report.status).toBe('failed');
      expect(report.missing.some((s) => s === adapter.inputSelector)).toBe(false);
      expect(report.missing.some((s) => s === adapter.sendSelector)).toBe(true);
    });

    it(`${adapter.id}: failed при семантически непригодных элементах`, () => {
      const selectorToElement: Record<string, PageElement | null> = {
        [adapter.inputSelector]: element({
          tagName: adapter.inputKind === 'textarea' ? 'TEXTAREA' : 'DIV',
          editable: false,
        }),
        [adapter.sendSelector]: element({ tagName: 'BUTTON', connected: false }),
        [adapter.mountSelector]: element({
          tagName: adapter.id === 'gemini' ? 'DIV' : 'FORM',
        }),
      };
      const report = adapter.probe(fakePage(selectorToElement));
      expect(report.status).toBe('failed');
      expect(report.missing).toContain(`${adapter.inputSelector} (editable)`);
      expect(report.missing.some((problem) => problem.startsWith(adapter.sendSelector))).toBe(true);
    });
  }
});

describe('isSendEvent', () => {
  for (const adapter of ADAPTERS) {
    it(`${adapter.id}: Enter без модификаторов отправляет`, () => {
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Enter' })).toBe(true);
    });

    it(`${adapter.id}: Enter с Shift/Ctrl/Meta/Alt — нет`, () => {
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Enter', shiftKey: true })).toBe(false);
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Enter', ctrlKey: true })).toBe(false);
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Enter', metaKey: true })).toBe(false);
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Enter', altKey: true })).toBe(false);
    });

    it(`${adapter.id}: Enter при IME-композиции — нет`, () => {
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Enter', isComposing: true })).toBe(false);
    });

    it(`${adapter.id}: другие клавиши — нет`, () => {
      expect(adapter.isSendEvent({ type: 'keydown', key: 'a' })).toBe(false);
      expect(adapter.isSendEvent({ type: 'keydown', key: 'Tab' })).toBe(false);
    });

    it(`${adapter.id}: клик по кнопке Send — да`, () => {
      expect(
        adapter.isSendEvent({ type: 'click', targetTagName: 'BUTTON', targetAriaLabel: 'Send message' }),
      ).toBe(true);
    });

    it(`${adapter.id}: клик по не-кнопке — нет`, () => {
      expect(adapter.isSendEvent({ type: 'click', targetTagName: 'DIV' })).toBe(false);
      expect(adapter.isSendEvent({ type: 'click', targetTagName: 'BUTTON', targetAriaLabel: 'Cancel' })).toBe(false);
    });
  }
});
