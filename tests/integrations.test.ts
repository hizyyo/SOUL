import { describe, it, expect } from 'vitest';
import {
  isClientId,
  clientLabel,
  clientStateLabel,
  clientAction,
  canConnect,
  canDisconnect,
  clientStatusNote,
  clientActionLabel,
  type ClientStatus,
} from '../src/data/integrations';

function status(over: Partial<ClientStatus> = {}): ClientStatus {
  return {
    client: 'claude-code',
    label: 'Claude Code',
    config_path: 'C:/Users/test/.claude.json',
    config_exists: true,
    connected: false,
    server_binary: 'C:/App/soul-mcp.exe',
    server_binary_exists: true,
    backup_path: null,
    error: null,
    ...over,
  };
}

describe('client identity', () => {
  it('knows the three supported clients', () => {
    expect(isClientId('claude-code')).toBe(true);
    expect(isClientId('codex')).toBe(true);
    expect(isClientId('cursor')).toBe(true);
    expect(isClientId('vim')).toBe(false);
  });

  it('maps ids to human labels', () => {
    expect(clientLabel('claude-code')).toBe('Claude Code');
    expect(clientLabel('cursor')).toBe('Cursor');
    expect(clientLabel('unknown')).toBe('unknown');
  });
});

describe('client state label', () => {
  it('marks connected clients first', () => {
    expect(clientStateLabel(status({ connected: true, error: 'x' }))).toBe('Connected');
  });

  it('surfaces errors over detection', () => {
    expect(clientStateLabel(status({ error: 'invalid json' }))).toBe('Error');
  });

  it('distinguishes missing config', () => {
    expect(clientStateLabel(status({ config_exists: false }))).toBe('Not found');
    expect(clientStateLabel(status({}))).toBe('Detected');
  });
});

describe('client actions', () => {
  it('offers connect when config exists or can be created', () => {
    expect(clientAction(status({}))).toBe('connect');
    expect(clientAction(status({ config_exists: false }))).toBe('connect');
    expect(canConnect(status({}))).toBe(true);
  });

  it('offers disconnect when connected', () => {
    expect(clientAction(status({ connected: true }))).toBe('disconnect');
    expect(canDisconnect(status({ connected: true }))).toBe(true);
  });

  it('offers nothing when config is broken', () => {
    expect(clientAction(status({ error: 'invalid json' }))).toBe('none');
    expect(canConnect(status({ error: 'invalid json' }))).toBe(false);
  });

  it('labels actions for the button', () => {
    expect(clientActionLabel(status({}))).toBe('Connect');
    expect(clientActionLabel(status({ connected: true }))).toBe('Disconnect');
    expect(clientActionLabel(status({ error: 'broken' }))).toBe('Rollback');
  });
});

describe('status note', () => {
  it('explains connected state', () => {
    const note = clientStatusNote(status({ connected: true }));
    expect(note).toContain('Connected to this local MCP server');
  });

  it('explains errors', () => {
    const note = clientStatusNote(status({ error: 'cannot parse' }));
    expect(note).toContain('Cannot modify config: cannot parse');
  });

  it('mentions binary problems', () => {
    const note = clientStatusNote(status({ server_binary_exists: false }));
    expect(note).toContain('server binary not found');
  });

  it('mentions the backup path when present', () => {
    const note = clientStatusNote(status({ backup_path: 'C:/x/backup.json' }));
    expect(note).toContain('Backup: C:/x/backup.json');
  });
});
