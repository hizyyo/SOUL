/**
 * Модель интеграций с coding-клиентами.
 *
 * Чистые функции для UI: статусы, подписи и действия вычисляются
 * детерминированно из ClientStatus, который возвращает Rust-бэкенд
 * (detect/connect/disconnect/rollback). Сами операции — только в бэкенде;
 * здесь ничего не пишется и не читается.
 */

export type ClientId = 'claude-code' | 'codex' | 'cursor';

export const CLIENT_IDS: readonly ClientId[] = ['claude-code', 'codex', 'cursor'];

export const CLIENT_LABELS: Record<ClientId, string> = {
  'claude-code': 'Claude Code',
  codex: 'Codex',
  cursor: 'Cursor',
};

export interface ClientStatus {
  client: string;
  label: string;
  config_path: string;
  config_exists: boolean;
  connected: boolean;
  server_binary: string;
  server_binary_exists: boolean;
  backup_path: string | null;
  error: string | null;
}

export function isClientId(value: string): value is ClientId {
  return CLIENT_IDS.includes(value as ClientId);
}

export function clientLabel(client: string): string {
  return isClientId(client) ? CLIENT_LABELS[client] : client;
}

export type ClientStateLabel = 'Connected' | 'Detected' | 'Not found' | 'Error';

/** Краткий статус для бейджа в UI. */
export function clientStateLabel(status: ClientStatus): ClientStateLabel {
  if (status.connected) return 'Connected';
  if (status.error) return 'Error';
  if (!status.config_exists) return 'Not found';
  return 'Detected';
}

export type ClientAction = 'connect' | 'disconnect' | 'none';

/** Действие, доступное пользователю для клиента. */
export function clientAction(status: ClientStatus): ClientAction {
  if (status.connected) return 'disconnect';
  if (status.error || status.config_path === '') return 'none';
  return 'connect';
}

export function canConnect(status: ClientStatus): boolean {
  return clientAction(status) === 'connect';
}

export function canDisconnect(status: ClientStatus): boolean {
  return clientAction(status) === 'disconnect';
}

/** Человекочитаемое описание статуса для подписи под карточкой. */
export function clientStatusNote(status: ClientStatus): string {
  const parts: string[] = [];
  if (status.connected) {
    parts.push('Connected to this local MCP server.');
  } else if (status.error) {
    parts.push(`Cannot modify config: ${status.error}`);
  } else if (!status.config_exists) {
    parts.push('Config file not found. Connect will create it.');
  } else {
    parts.push('Detected. Config file is present.');
  }
  if (!status.server_binary_exists) {
    parts.push('MCP server binary not found next to the app.');
  }
  if (status.backup_path) {
    parts.push(`Backup: ${status.backup_path}`);
  }
  return parts.join(' ');
}

export function clientActionLabel(status: ClientStatus): string {
  switch (clientAction(status)) {
    case 'connect':
      return 'Connect';
    case 'disconnect':
      return 'Disconnect';
    case 'none':
      return status.error ? 'Rollback' : '—';
  }
}
