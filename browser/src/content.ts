/**
 * Content script: перехватывает один пользовательский Send, запрашивает
 * разрешённый контекст у host-а, добавляет структурированный SOUL context
 * в то же сообщение и продолжает отправку без второго клика.
 */

import {
  DEFAULT_TOKENS,
  EXTENSION_ID,
  PROTOCOL_VERSION,
  SUPPORTED_ORIGINS,
} from './constants';
import { createNonce } from './nonce';
import { isContextResponse, isErrorResponse, type ErrorResponse } from './protocol';
import { chipLabel, composeMessage, SOUL_BLOCK_START } from './compose';
import type { AdapterEvent, Page, SiteAdapter } from './adapters/types';
import { adapterForOrigin } from './adapters/registry';
import { createPageFromDocument, setContenteditableValue, setTextareaValue } from './dom';
import { createChip, type ChipController, type ChipState } from './ui/chip';
import { closestMatchingTarget } from './interception';
import { collapsibleUserText } from './history';
import { createLifecycleController } from './lifecycle';
import {
  createSubmissionSnapshot,
  submissionStillMatches,
  type SubmissionSnapshot,
} from './submission';

const origin = window.location.origin;
if (!SUPPORTED_ORIGINS.includes(origin)) {
  throw new Error('SOUL Browser Companion: сайт не поддерживается.');
}

function loadAdapter(): SiteAdapter {
  const candidate = adapterForOrigin(origin);
  if (!candidate) {
    throw new Error('SOUL Browser Companion: нет адаптера для сайта.');
  }
  return candidate;
}

const adapter = loadAdapter();
const RESUME_DELAY_MS = 40;
const RECONCILE_INTERVAL_MS = 1_000;
const MOUNT_GRACE_MS = 5_000;
const MAX_HISTORY_MESSAGES_PER_SWEEP = 100;

const state = {
  /** fail-closed: текущая разметка не прошла probe. */
  disabled: true,
  /** Все семантические элементы текущего composer-а доступны. */
  ready: false,
  /** Идёт запрос контекста; повторные Send подавляются. */
  busy: false,
  /** Пропустить контекст для следующего непустого сообщения. */
  oneMessageOff: false,
  /** Аварийный выключатель сессии. */
  sessionOff: false,
  /** Защита от повторного перехвата программной отправки. */
  resuming: false,
  /** Не считать input-события собственного verified setter-а правкой пользователя. */
  mutating: false,
};

let lastCount: number | null = null;
let missingSince: number | null = null;
let submissionGeneration = 0;
let chip: ChipController | null = null;
let activeSubmission:
  | {
      generation: number;
      input: HTMLElement;
      snapshot: SubmissionSnapshot;
      expectedDraft: string;
    }
  | null = null;

function makePage(): Page {
  return createPageFromDocument(document, origin, window.location.href);
}

function currentInput(): HTMLElement | null {
  return document.querySelector<HTMLElement>(adapter.inputSelector);
}

function readDraft(input: HTMLElement): string {
  return input instanceof HTMLTextAreaElement ? input.value : (input.textContent ?? '');
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
  if (isContextResponse(response, nonce)) {
    return { pack: response.pack, entityCount: response.entityCount };
  }
  if (isErrorResponse(response)) {
    return response;
  }
  return { type: 'soul.error', code: 'runtime_error', message: 'Неожиданный ответ расширения.' };
}

function setInputValue(input: HTMLElement, text: string): boolean {
  state.mutating = true;
  try {
    return adapter.inputKind === 'textarea'
      ? setTextareaValue(input as HTMLTextAreaElement, text)
      : setContenteditableValue(input, text);
  } finally {
    state.mutating = false;
  }
}

function attemptSend(page: Page, input: HTMLElement): boolean {
  const button = page.querySelector(adapter.sendSelector);
  state.resuming = true;
  try {
    if (button?.tagName === 'BUTTON' && button.connected && button.enabled) {
      button.click();
      return true;
    }
    if (input.isConnected) {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      return true;
    }
    return false;
  } finally {
    state.resuming = false;
  }
}

function updateChip(update: { state: ChipState; count?: number | null; errorHint?: string }): void {
  chip?.update({
    state: update.state,
    count: update.count === undefined ? lastCount : update.count,
    oneMessageOff: state.oneMessageOff && !state.sessionOff && !state.disabled,
    sessionOff: state.sessionOff,
    ...(update.errorHint !== undefined ? { errorHint: update.errorHint } : {}),
  });
}

function snapshotStillMatches(input: HTMLElement, snapshot: SubmissionSnapshot): boolean {
  return (
    currentInput() === input &&
    submissionStillMatches(snapshot, {
      route: window.location.href,
      draft: readDraft(input),
      connected: input.isConnected,
    })
  );
}

function canResumeComposed(input: HTMLElement, snapshot: SubmissionSnapshot, text: string): boolean {
  return (
    currentInput() === input &&
    input.isConnected &&
    window.location.href === snapshot.route &&
    readDraft(input) === text
  );
}

function restoreDraftAfterStaleCompose(
  input: HTMLElement,
  snapshot: SubmissionSnapshot,
  composedText: string,
): void {
  if (
    currentInput() === input &&
    input.isConnected &&
    window.location.href === snapshot.route &&
    readDraft(input) === composedText
  ) {
    setInputValue(input, snapshot.draft);
  }
}

function abortStaleSubmission(generation: number): void {
  if (generation === submissionGeneration && !state.sessionOff) {
    updateChip({ state: state.disabled ? 'error' : 'on' });
  }
}

async function intercept(
  input: HTMLElement,
  snapshot: SubmissionSnapshot,
  generation: number,
): Promise<void> {
  try {
    const response = await fetchContext(snapshot.contextQuery);
    if (generation !== submissionGeneration || !snapshotStillMatches(input, snapshot)) {
      abortStaleSubmission(generation);
      return;
    }
    if (isErrorResponse(response)) {
      updateChip({ state: 'error', errorHint: response.message });
      if (!attemptSend(makePage(), input)) {
        updateChip({
          state: 'error',
          errorHint: 'Кнопка отправки не найдена: сообщение не отправлено.',
        });
      }
      return;
    }

    const composed = composeMessage(snapshot.draft, response.pack);
    if (!setInputValue(input, composed.text)) {
      updateChip({
        state: 'error',
        errorHint: 'Сайт отклонил изменение поля ввода: сообщение не отправлено.',
      });
      return;
    }
    if (activeSubmission?.generation === generation) {
      activeSubmission.expectedDraft = composed.text;
    }
    lastCount = response.entityCount;
    updateChip({ state: 'on', count: response.entityCount });
    await new Promise((resolve) => globalThis.setTimeout(resolve, RESUME_DELAY_MS));
    if (
      generation !== submissionGeneration ||
      !canResumeComposed(input, snapshot, composed.text)
    ) {
      restoreDraftAfterStaleCompose(input, snapshot, composed.text);
      abortStaleSubmission(generation);
      return;
    }
    if (!attemptSend(makePage(), input)) {
      updateChip({
        state: 'error',
        errorHint: 'Кнопка отправки не найдена: сообщение не отправлено.',
      });
    }
  } finally {
    if (generation === submissionGeneration) {
      state.busy = false;
      activeSubmission = null;
    }
  }
}

function beginInterception(input: HTMLElement): boolean {
  const snapshot = createSubmissionSnapshot(window.location.href, readDraft(input));
  if (!snapshot) {
    return false;
  }
  if (state.oneMessageOff) {
    state.oneMessageOff = false;
    updateChip({ state: 'on' });
    return false;
  }
  state.busy = true;
  const generation = ++submissionGeneration;
  activeSubmission = { generation, input, snapshot, expectedDraft: snapshot.draft };
  updateChip({ state: 'loading' });
  void intercept(input, snapshot, generation);
  return true;
}

function cancelActiveSubmission(): void {
  if (!activeSubmission) {
    return;
  }
  submissionGeneration += 1;
  activeSubmission = null;
  state.busy = false;
  updateChip({ state: state.disabled ? 'error' : state.sessionOff ? 'off' : 'on' });
}

function onComposerInput(event: Event): void {
  if (state.mutating || !activeSubmission) {
    return;
  }
  const matched = closestMatchingTarget(event.target, adapter.inputSelector);
  if (
    matched === activeSubmission.input &&
    readDraft(activeSubmission.input) !== activeSubmission.expectedDraft
  ) {
    cancelActiveSubmission();
  }
}

function onKeydown(event: KeyboardEvent): void {
  if (state.disabled || !state.ready || state.sessionOff) {
    return;
  }
  const matched = closestMatchingTarget(event.target, adapter.inputSelector);
  if (!matched || !(matched instanceof HTMLElement)) {
    return;
  }
  if (state.resuming && event.key === 'Enter') {
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
  if (state.busy) {
    event.preventDefault();
    event.stopImmediatePropagation();
    return;
  }
  if (beginInterception(matched)) {
    event.preventDefault();
    event.stopImmediatePropagation();
  }
}

function onSendButtonClick(event: Event): void {
  if (state.disabled || !state.ready || state.sessionOff) {
    return;
  }
  if (!closestMatchingTarget(event.target, adapter.sendSelector)) {
    return;
  }
  if (state.resuming) {
    return;
  }
  if (state.busy) {
    event.preventDefault();
    event.stopImmediatePropagation();
    return;
  }
  const input = currentInput();
  if (input && beginInterception(input)) {
    event.preventDefault();
    event.stopImmediatePropagation();
  }
}

function collapseUserMessage(message: Element): void {
  if (message.hasAttribute('data-soul-collapsed')) {
    return;
  }
  const messageText = message.textContent ?? '';
  const result = collapsibleUserText(messageText, true);
  if (!result) {
    return;
  }
  const details = document.createElement('details');
  details.setAttribute('data-soul-history-chip', '');
  const summary = document.createElement('summary');
  summary.textContent = chipLabel(result.count);
  summary.setAttribute('aria-label', `${chipLabel(result.count)}; expand to view`);
  const pre = document.createElement('pre');
  pre.textContent = result.block;
  details.append(summary, pre);
  if (!replaceTextRange(message, result.userText.length, messageText.length, details)) {
    return;
  }
  message.setAttribute('data-soul-collapsed', '');
}

function replaceTextRange(
  root: Element,
  startOffset: number,
  endOffset: number,
  replacement: Node,
): boolean {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const range = document.createRange();
  let position = 0;
  let startSet = false;
  let node: Node | null;
  while ((node = walker.nextNode()) !== null) {
    const text = node as Text;
    const nextPosition = position + text.data.length;
    if (!startSet && startOffset >= position && startOffset <= nextPosition) {
      range.setStart(text, startOffset - position);
      startSet = true;
    }
    if (startSet && endOffset >= position && endOffset <= nextPosition) {
      range.setEnd(text, endOffset - position);
      range.deleteContents();
      range.insertNode(replacement);
      return true;
    }
    position = nextPosition;
  }
  return false;
}

function sweepHistory(): void {
  const history = document.querySelector(adapter.historySelector);
  if (!history) {
    return;
  }
  const messages = history.querySelectorAll(adapter.userMessageSelector);
  let swept = 0;
  for (const message of messages) {
    if (swept >= MAX_HISTORY_MESSAGES_PER_SWEEP) {
      break;
    }
    swept += 1;
    if (
      (message.textContent ?? '').includes(SOUL_BLOCK_START) &&
      message.querySelector('[data-soul-history-chip]') === null
    ) {
      collapseUserMessage(message);
    }
  }
}

function mountChip(): void {
  if (!chip) {
    return;
  }
  const mount = document.querySelector(adapter.mountSelector);
  const parent = mount?.parentElement;
  if (!mount || !parent) {
    return;
  }
  if (chip.host.parentElement !== parent || chip.host.previousElementSibling !== mount) {
    parent.insertBefore(chip.host, mount.nextSibling);
  }
}

function reconcile(): void {
  mountChip();
  sweepHistory();

  if (
    activeSubmission &&
    (currentInput() !== activeSubmission.input ||
      !activeSubmission.input.isConnected ||
      window.location.href !== activeSubmission.snapshot.route ||
      readDraft(activeSubmission.input) !== activeSubmission.expectedDraft)
  ) {
    cancelActiveSubmission();
  }

  const page = makePage();
  const input = page.querySelector(adapter.inputSelector);
  const send = page.querySelector(adapter.sendSelector);
  const mount = page.querySelector(adapter.mountSelector);
  if (!input || !send || !mount) {
    state.ready = false;
    if (missingSince === null) {
      missingSince = Date.now();
    }
    if (Date.now() - missingSince < MOUNT_GRACE_MS) {
      state.disabled = false;
      updateChip({ state: 'loading' });
      return;
    }
  } else {
    missingSince = null;
  }

  const report = adapter.probe(page);
  if (report.status === 'failed') {
    state.disabled = true;
    state.ready = false;
    updateChip({
      state: 'error',
      errorHint: `Разметка ${adapter.label} не распознана (не найдено: ${report.missing.join(', ')}): SOUL отключён.`,
    });
    return;
  }

  state.disabled = false;
  state.ready = true;
  updateChip({ state: state.sessionOff ? 'off' : state.busy ? 'loading' : 'on' });
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
    cancelActiveSubmission();
    state.sessionOff = true;
    state.oneMessageOff = false;
    updateChip({ state: 'off' });
  },
  onChipClick() {
    if (state.sessionOff) {
      state.sessionOff = false;
      reconcile();
    }
  },
});

window.addEventListener('keydown', onKeydown, true);
document.addEventListener('click', onSendButtonClick, true);
document.addEventListener('input', onComposerInput, true);

const lifecycle = createLifecycleController(
  reconcile,
  {
    observe(listener) {
      const observer = new MutationObserver(listener);
      observer.observe(document.documentElement, {
        childList: true,
        subtree: true,
      });
      return () => observer.disconnect();
    },
    setInterval(listener, intervalMs) {
      return globalThis.setInterval(listener, intervalMs);
    },
    clearInterval(handle) {
      globalThis.clearInterval(handle as ReturnType<typeof setInterval>);
    },
  },
  RECONCILE_INTERVAL_MS,
);

function cleanup(): void {
  submissionGeneration += 1;
  lifecycle.destroy();
  window.removeEventListener('keydown', onKeydown, true);
  document.removeEventListener('click', onSendButtonClick, true);
  document.removeEventListener('input', onComposerInput, true);
  chip?.destroy();
  chip = null;
}

window.addEventListener(
  'pagehide',
  (event) => {
    if (!event.persisted) {
      cleanup();
    }
  },
);
