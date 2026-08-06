/**
 * Чип SOUL рядом с полем ввода: индикатор состояния, число выбранных
 * сущностей, выключатель «1 сообщение» и аварийный выключатель сессии.
 * Стили изолированы через shadow root.
 */

export type ChipState = 'on' | 'off' | 'loading' | 'error';

export interface ChipUpdate {
  state: ChipState;
  /** Число сущностей в последнем контексте (null — неизвестно). */
  count: number | null;
  /** Включён ли пропуск следующего сообщения. */
  oneMessageOff: boolean;
  /** Выключена ли сессия (аварийный выключатель). */
  sessionOff: boolean;
  /** Подпись для ошибки (например, причина fail-closed). */
  errorHint?: string;
}

export interface ChipCallbacks {
  /** Клик по «1 msg»: пропустить контекст для одного сообщения. */
  onToggleOneMessage(): void;
  /** Клик по «×»: аварийно выключить сессию (без контекста). */
  onSessionOff(): void;
  /** Клик по чипу: переключить пропуск одного сообщения. */
  onChipClick(): void;
}

export interface ChipController {
  readonly host: HTMLElement;
  update(update: ChipUpdate): void;
  destroy(): void;
}

const LABELS: Record<ChipState, string> = {
  on: 'SOUL ON',
  off: 'SOUL OFF',
  loading: 'SOUL …',
  error: 'SOUL ERR',
};

export interface ChipViewModel {
  readonly active: boolean;
  readonly primaryLabel: string;
  readonly primaryAriaLabel: string;
  readonly oneMessageVisible: boolean;
  readonly sessionOffVisible: boolean;
}

export function chipViewModel(update: ChipUpdate): ChipViewModel {
  const active = update.state === 'on' && !update.sessionOff;
  const canEnable = update.sessionOff || update.state === 'off';
  return {
    active,
    primaryLabel: LABELS[update.state],
    primaryAriaLabel: canEnable
      ? 'Enable SOUL'
      : update.state === 'error'
        ? `SOUL error${update.errorHint ? `: ${update.errorHint}` : ''}`
        : `SOUL status: ${LABELS[update.state]}`,
    oneMessageVisible: active,
    sessionOffVisible: active,
  };
}

export function createChip(callbacks: ChipCallbacks): ChipController {
  const host = document.createElement('div');
  host.setAttribute('data-soul-chip', '');
  host.setAttribute('role', 'group');
  host.setAttribute('aria-label', 'SOUL Browser Companion');
  host.style.cssText =
    'display:inline-flex;align-items:center;gap:6px;font:12px/1.4 system-ui,sans-serif;' +
    'color:#667;user-select:none;margin:4px 0;';

  const shadow = host.attachShadow({ mode: 'open' });

  const primaryButton = document.createElement('button');
  primaryButton.type = 'button';
  primaryButton.style.cssText =
    'display:inline-flex;align-items:center;gap:6px;border:0;background:transparent;padding:1px;' +
    'font:inherit;color:inherit;cursor:pointer;';
  primaryButton.addEventListener('click', () => callbacks.onChipClick());
  shadow.appendChild(primaryButton);

  const dot = document.createElement('span');
  dot.style.cssText = 'width:8px;height:8px;border-radius:50%;background:#aaa;flex:none;';
  dot.setAttribute('aria-hidden', 'true');
  primaryButton.appendChild(dot);

  const label = document.createElement('span');
  label.style.cssText = 'font-weight:600;white-space:nowrap;';
  label.setAttribute('role', 'status');
  label.setAttribute('aria-live', 'polite');
  primaryButton.appendChild(label);

  const countSpan = document.createElement('span');
  countSpan.style.cssText = 'color:#889;white-space:nowrap;display:none;';
  primaryButton.appendChild(countSpan);

  const oneMsgButton = document.createElement('button');
  oneMsgButton.textContent = '1 msg';
  oneMsgButton.type = 'button';
  oneMsgButton.style.cssText =
    'border:1px solid #ccc;background:#fff;border-radius:10px;padding:1px 8px;' +
    'font:inherit;color:#556;cursor:pointer;';
  oneMsgButton.addEventListener('click', (event) => {
    event.stopPropagation();
    callbacks.onToggleOneMessage();
  });
  oneMsgButton.setAttribute('aria-label', 'Skip SOUL context for the next message');
  shadow.appendChild(oneMsgButton);

  const sessionOffButton = document.createElement('button');
  sessionOffButton.textContent = '×';
  sessionOffButton.type = 'button';
  sessionOffButton.title = 'Выключить SOUL до перезагрузки страницы';
  sessionOffButton.setAttribute('aria-label', 'Disable SOUL for this session');
  sessionOffButton.style.cssText =
    'border:1px solid #d88;background:#fff;border-radius:10px;padding:1px 7px;' +
    'font:inherit;color:#c33;cursor:pointer;';
  sessionOffButton.addEventListener('click', (event) => {
    event.stopPropagation();
    callbacks.onSessionOff();
  });
  shadow.appendChild(sessionOffButton);

  const controller: ChipController = {
    host,
    update(update: ChipUpdate): void {
      const view = chipViewModel(update);
      label.textContent = view.primaryLabel;
      dot.style.background =
        update.state === 'on'
          ? '#2a9d4e'
          : update.state === 'error'
            ? '#d33'
            : update.state === 'loading'
              ? '#b90'
              : '#999';
      if (update.state === 'error') {
        label.title = update.errorHint ?? 'Разметка сайта изменилась: SOUL отключён.';
      } else {
        label.title = '';
      }
      if (update.count !== null && update.count > 0) {
        countSpan.textContent = `· ${update.count} items`;
        countSpan.style.display = '';
      } else {
        countSpan.textContent = '';
        countSpan.style.display = 'none';
      }
      oneMsgButton.style.display = view.oneMessageVisible ? '' : 'none';
      oneMsgButton.style.borderColor = update.oneMessageOff ? '#2a9d4e' : '#ccc';
      oneMsgButton.textContent = update.oneMessageOff ? '1 msg OFF' : '1 msg';
      oneMsgButton.setAttribute('aria-pressed', String(update.oneMessageOff));
      sessionOffButton.style.display = view.sessionOffVisible ? '' : 'none';
      primaryButton.setAttribute('aria-pressed', String(view.active));
      primaryButton.setAttribute('aria-label', view.primaryAriaLabel);
      primaryButton.disabled = update.state === 'loading';
      host.dataset.state = update.state;
    },
    destroy(): void {
      host.remove();
    },
  };

  controller.update({ state: 'loading', count: null, oneMessageOff: false, sessionOff: false });
  return controller;
}
