/**
 * Content script: перехватывает один пользовательский Send, запрашивает
 * разрешённый контекст у host-а, добавляет структурированный SOUL context
 * в то же сообщение и продолжает отправку без второго клика.
 *
 * Безопасность: контекст не сохраняется; при изменении разметки сайта
 * адаптер отключается (fail-closed); при сбое host-а сообщение уходит
 * без контекста (fail-open), как обычное.
 */

import {
  DEFAULT_TOKENS,
  EXTENSION_ID,
  MAX_TASK_CHARS,
  PROTOCOL_VERSION,
  SUPPORTED_ORIGINS,
} from './constants';
import { createNonce } from './nonce';
import { isErrorResponse, type ErrorResponse } from './protocol';
import { chipLabel, collapseText, composeMessage, SOUL_BLOCK_START } from './compose';
import type { AdapterEvent, Page, SiteAdapter } from './adapters/types';
import { adapterForOrigin } from './adapters/registry';
import { createPageFromDocument, setContenteditableValue, setTextareaValue } from './dom';
import { createChip, type ChipController, type ChipState } from './ui/chip';

const origin = window.location.origin;

if (!SUPPORTED_ORIGINS.includes(origin)) {
  throw new Error('SOUL Browser Companion: сайт не поддерживается.');
}
/** Адаптер для текущего сайта или исключение (сайт не поддерживается). */
function loadAdapter(): SiteAdapter {
  const candidate = adapterForOrigin(origin);
  if (!candidate) {
    throw new Error('SOUL Browser Companion: нет адаптера для сайта.');
  }
  return candidate;
}

const adapter = loadAdapter();

const HEALTH_INTERVAL_MS = 30_000;
const RESUME_DELAY_MS = 40;

const state = {
  /** fail-closed: разметка изменилась, перехват отключён. */
  disabled: false,
  /** идёт запрос контекста; повторные Send подавляются. */
  busy: false,
  /** пропустить контекст для следующего сообщения. */
  oneMessageOff: false,
  /** аварийный выключатель сессии. */
  sessionOff: false,
  /** resuming-флаг для программной отправки (защита от петли). */
  resuming: false,
};

let lastCount: number | null = null;
let chip: ChipController | null = null;

function makePage(): Page {
  return createPageFromDocument(document, origin, window.location.href);
}

function readTask(page: Page): string {
  const input = page.querySelector(adapter.inputSelector);
  if (!input) {
    return '';
  }
  return input.value.slice(0, MAX_TASK_CHARS);
}

async function fetchContext(
  task: string,
): Promise<{ pack: string; entityCount: number } | ErrorResponse> {
  const nonce = createNonce();
  if (!nonce) {
    return { type: 'soul.error', code: 'runtime_error', message: 'Нет crypto.getRandomValues.' };
  }
  let response: unknown;
  try {
    response = await chrome.runtime.sendMessage({
      type: 'soul.get_context',
      protocol: PROTOCOL_VERSION,
      extensionId: EXTENSION_ID,
      nonce,
      origin,
      task,
      maxTokens: DEFAULT_TOKENS,
    });
  } catch (error) {
    return {
      type: 'soul.error',
      code: 'runtime_error',
      message: `Расширение не ответило: ${String(error)}`,
    };
  }
  if (isErrorResponse(response)) {
    return response;
  }
  if (
    typeof response === 'object' &&
    response !== null &&
    (response as { type?: unknown }).type === 'soul.context' &&
    typeof (response as { pack?: unknown }).pack === 'string'
  ) {
    const context = response as { pack: string; entityCount: number };
    return { pack: context.pack, entityCount: context.entityCount };
  }
  return { type: 'soul.error', code: 'runtime_error', message: 'Неожиданный ответ расширения.' };
}

function setInputValue(input: HTMLElement | null, text: string): void {
  if (!input) {
    return;
  }
  if (adapter.inputKind === 'textarea') {
    setTextareaValue(input as HTMLTextAreaElement, text);
  } else {
    setContenteditableValue(input, text);
  }
}

/** Программная отправка: клик по кнопке Send, иначе Enter в поле ввода. */
function attemptSend(page: Page, input: HTMLElement | null): boolean {
  const button = page.querySelector(adapter.sendSelector);
  if (button && button.tagName === 'BUTTON') {
    state.resuming = true;
    button.click();
    return true;
  }
  if (input) {
    state.resuming = true;
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    return true;
  }
  return false;
}

function updateChip(update: { state: ChipState; count?: number | null; errorHint?: string }): void {
  if (!chip) {
    return;
  }
  chip.update({
    state: update.state,
    count: update.count === undefined ? lastCount : update.count,
    oneMessageOff: state.oneMessageOff && !state.sessionOff && !state.disabled,
    sessionOff: state.sessionOff,
    ...(update.errorHint !== undefined ? { errorHint: update.errorHint } : {}),
  });
}

async function intercept(): Promise<void> {
  state.busy = true;
  updateChip({ state: 'loading' });
  try {
    const page = makePage();
    const input = document.querySelector<HTMLElement>(adapter.inputSelector);
    const task = readTask(page);
    const response = await fetchContext(task);
    if (isErrorResponse(response)) {
      // fail-open: без контекста отправляем обычное сообщение.
      updateChip({ state: 'error', errorHint: response.message });
      if (!attemptSend(page, input)) {
        updateChip({
          state: 'error',
          errorHint: 'Кнопка отправки не найдена: сообщение не отправлено.',
        });
      }
      return;
    }
    lastCount = response.entityCount;
    const composed = composeMessage(task, response.pack);
    setInputValue(input, composed.text);
    updateChip({ state: 'on', count: response.entityCount });
    await new Promise((resolve) => setTimeout(resolve, RESUME_DELAY_MS));
    if (!attemptSend(page, input)) {
      updateChip({
        state: 'error',
        errorHint: 'Кнопка отправки не найдена: сообщение не отправлено.',
      });
    }
  } finally {
    state.busy = false;
  }
}

function onKeydown(event: KeyboardEvent): void {
  if (state.disabled || state.sessionOff) {
    return;
  }
  if (state.resuming) {
    state.resuming = false;
    if (event.key === 'Enter') {
      return;
    }
  }
  if (state.busy) {
    event.preventDefault();
    event.stopImmediatePropagation();
    return;
  }
  const candidate: AdapterEvent = {
    type: 'keydown',
    key: event.key,
    shiftKey: event.shiftKey,
    ctrlKey: event.ctrlKey,
    metaKey: event.metaKey,
    altKey: event.altKey,
    isComposing: event.isComposing,
  };
  if (!adapter.isSendEvent(candidate)) {
    return;
  }
  if (state.oneMessageOff) {
    state.oneMessageOff = false;
    updateChip({ state: 'on' });
    return;
  }
  event.preventDefault();
  event.stopImmediatePropagation();
  void intercept();
}

function onSendButtonClick(event: Event): void {
  if (state.disabled || state.sessionOff) {
    return;
  }
  if (state.resuming) {
    state.resuming = false;
    return;
  }
  if (state.busy) {
    event.preventDefault();
    event.stopImmediatePropagation();
    return;
  }
  if (state.oneMessageOff) {
    state.oneMessageOff = false;
    updateChip({ state: 'on' });
    return;
  }
  event.preventDefault();
  event.stopImmediatePropagation();
  void intercept();
}

function runHealthCheck(): void {
  if (state.disabled) {
    return;
  }
  const report = adapter.probe(makePage());
  if (report.status === 'failed') {
    state.disabled = true;
    updateChip({
      state: 'error',
      errorHint: `Разметка ${adapter.label} изменилась (не найдено: ${report.missing.join(', ')}): SOUL отключён.`,
    });
  }
}

/** Сворачивает технический блок в истории в раскрываемый чип. */
function collapseTextInNode(node: Text): void {
  const result = collapseText(node.data);
  const parent = node.parentElement;
  if (!result || !parent) {
    return;
  }
  const chipElement = document.createElement('details');
  chipElement.setAttribute('data-soul-chip', '');
  const summary = document.createElement('summary');
  summary.textContent = chipLabel(result.count);
  const pre = document.createElement('pre');
  pre.textContent = result.block;
  chipElement.append(summary, pre);
  node.data = result.userText;
  parent.insertBefore(chipElement, node.nextSibling);
  if (result.rest.trim() !== '') {
    parent.insertBefore(document.createTextNode(result.rest), chipElement.nextSibling);
  }
}

function startCollapseWatcher(): void {
  const history = document.querySelector(adapter.historySelector) ?? document.body;
  const sweep = (): void => {
    if (state.disabled) {
      return;
    }
    const walker = document.createTreeWalker(history, NodeFilter.SHOW_TEXT);
    let node: Node | null;
    let scanned = 0;
    while (scanned < 400 && (node = walker.nextNode()) !== null) {
      scanned += 1;
      const textNode = node as Text;
      if (!textNode.data.includes(SOUL_BLOCK_START)) {
        continue;
      }
      const parent = textNode.parentElement;
      if (!parent || parent.closest('[data-soul-chip]')) {
        continue;
      }
      collapseTextInNode(textNode);
    }
  };
  const observer = new MutationObserver(sweep);
  observer.observe(history, { childList: true, subtree: true, characterData: true });
  window.setInterval(sweep, 4000);
}

function mountChip(chipController: ChipController): void {
  const mount = document.querySelector(adapter.mountSelector);
  if (!mount) {
    return;
  }
  mount.parentElement?.insertBefore(chipController.host, mount.nextSibling);
}

chip = createChip({
  onToggleOneMessage() {
    if (state.disabled || state.sessionOff) {
      return;
    }
    state.oneMessageOff = !state.oneMessageOff;
    updateChip({ state: 'on' });
  },
  onSessionOff() {
    state.sessionOff = true;
    state.oneMessageOff = false;
    updateChip({ state: 'off' });
  },
  onChipClick() {
    if (state.sessionOff) {
      state.sessionOff = false;
      updateChip({ state: 'on' });
    }
  },
});
mountChip(chip);

const initialHealth = adapter.probe(makePage());
if (initialHealth.status === 'ok') {
  updateChip({ state: 'on' });
  window.addEventListener('keydown', onKeydown, true);
  document.querySelector(adapter.sendSelector)?.addEventListener('click', onSendButtonClick);
  window.setInterval(runHealthCheck, HEALTH_INTERVAL_MS);
  startCollapseWatcher();
} else {
  state.disabled = true;
  updateChip({
    state: 'error',
    errorHint: `Разметка сайта не распознана (${initialHealth.missing.join(', ')}): SOUL отключён.`,
  });
}
