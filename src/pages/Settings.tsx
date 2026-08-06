import { useCallback, useEffect, useState } from 'react';
import {
  CLIENT_IDS,
  clientStateLabel,
  clientStatusNote,
  type ClientStatus,
} from '../data/integrations';
import {
  bridgeStateLabel,
  bridgeStatusNote,
  COMPANION_SITES,
  type BridgeStatus,
} from '../data/bridge';
import { safeErrorMessage } from '../data/safeError';
import { B1_PROFILE_STORAGE_KEY } from '../data/eval';
import { Modal } from '../components/Modal';

interface SoulInfo {
  soul_id: string;
  display_name: string;
  activated: boolean;
  calibration_step: number;
  entity_count: number;
  format_version: string;
  schema_version: string;
  created_at: string;
  head_event_hash: string | null;
  device_id: string;
}

interface EntityInfo {
  id: string;
  soul_id: string;
  entity_type: string;
  status: string;
  data: string;
  created_at: string;
  updated_at: string;
}

interface ExportReceipt {
  path: string;
  soul_id: string;
  display_name: string;
  entity_count: number;
  event_count: number;
  content_hash: string;
  signature: string;
  size_bytes: number;
  created_at: string;
}

interface FileReceipt {
  path: string;
  size_bytes: number;
}

interface ImportPreview {
  soul_id: string;
  display_name: string;
  created_at: string;
  schema_version: string;
  format_version: string;
  entity_count: number;
  event_count: number;
  calibration_step: number;
  activated: boolean;
  head_event_hash: string | null;
  entity_counts: { entity_type: string; count: number }[];
  partial_restore: boolean;
}

interface DeletionReceipt {
  deleted_at: string;
  entity_count: number;
  event_count: number;
  keys_deleted: boolean;
}

interface ReceiptSummary {
  file: string;
  kind: 'deletion' | 'disclosure';
  at: string;
  entity_count: number;
  event_count: number | null;
  keys_deleted: boolean | null;
  client: string | null;
  token_estimate: number | null;
  policy_version: string | null;
  state_version: string | null;
  cost_estimate_usd: number | null;
}

interface ContextUsageStats {
  disclosure_calls: number;
  input_tokens_total: number;
  cost_estimate_usd_total: number;
  last_disclosed_at: string | null;
}

type ModalState =
  | { kind: 'none' }
  | { kind: 'export-passphrase' }
  | { kind: 'restore-privacy' }
  | { kind: 'restore-passphrase' }
  | { kind: 'restore-preview'; token: string; password: string; preview: ImportPreview }
  | { kind: 'restore-done'; soulId: string }
  | { kind: 'delete-confirm' }
  | { kind: 'delete-receipt'; receipt: DeletionReceipt };

interface SettingsProps {
  soul: SoulInfo | null;
  entities: EntityInfo[];
  onDataChanged: () => void;
  onGoHome: () => void;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  }
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) throw new Error('Not running in Tauri');
  return internals.invoke(cmd, args) as Promise<T>;
}

export function Settings({ soul, entities, onDataChanged, onGoHome }: SettingsProps) {
  const [modal, setModal] = useState<ModalState>({ kind: 'none' });
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [receipts, setReceipts] = useState<ReceiptSummary[]>([]);
  const [usage, setUsage] = useState<ContextUsageStats | null>(null);
  const [clients, setClients] = useState<ClientStatus[]>([]);
  const [bridge, setBridge] = useState<BridgeStatus | null>(null);

  const loadReceipts = async () => {
    try {
      setReceipts(await invoke<ReceiptSummary[]>('list_receipts_cmd'));
      setUsage(await invoke<ContextUsageStats>('context_usage_cmd'));
    } catch {
      setReceipts([]);
    }
  };

  const loadClients = async () => {
    try {
      setClients(await invoke<ClientStatus[]>('detect_clients_cmd'));
    } catch {
      setClients([]);
    }
  };

  const loadBridge = async () => {
    try {
      setBridge(await invoke<BridgeStatus>('bridge_status_cmd'));
    } catch {
      setBridge(null);
    }
  };

  const handleBridgeRegister = async () => {
    setBusy('bridge');
    setError(null);
    setSuccess(null);
    try {
      setBridge(await invoke<BridgeStatus>('register_bridge_cmd'));
      setSuccess('Browser Companion host зарегистрирован для Chrome и Edge.');
    } catch {
      setError(safeErrorMessage('настроить Browser Companion'));
    } finally {
      setBusy(null);
    }
  };

  const handleBridgeUnregister = async () => {
    setBusy('bridge');
    setError(null);
    setSuccess(null);
    try {
      setBridge(await invoke<BridgeStatus>('unregister_bridge_cmd'));
      setSuccess('Browser Companion host отключён: веб-чаты работают как обычно.');
    } catch {
      setError(safeErrorMessage('отключить Browser Companion'));
    } finally {
      setBusy(null);
    }
  };

  useEffect(() => {
    void loadReceipts();
    void loadClients();
    void loadBridge();
  }, []);

  const handleConnect = async (client: string) => {
    setBusy(`Connecting ${client}...`);
    setError(null);
    try {
      await invoke<null>('connect_client_cmd', { client });
      setSuccess(`${client} connected to the local MCP server.`);
      await loadClients();
    } catch {
      setError(safeErrorMessage('подключить AI-клиент'));
      await loadClients();
    } finally {
      setBusy(null);
    }
  };

  const handleDisconnect = async (client: string) => {
    setBusy(`Disconnecting ${client}...`);
    setError(null);
    try {
      await invoke<null>('disconnect_client_cmd', { client });
      setSuccess(`${client} disconnected. The config was restored.`);
      await loadClients();
    } catch {
      setError(safeErrorMessage('отключить AI-клиент'));
      await loadClients();
    } finally {
      setBusy(null);
    }
  };

  const handleRollback = async (client: string) => {
    setBusy(`Rolling back ${client}...`);
    setError(null);
    try {
      await invoke<null>('rollback_client_cmd', { client });
      setSuccess(`${client} rolled back to the backup state.`);
      await loadClients();
    } catch {
      setError(safeErrorMessage('восстановить конфигурацию клиента'));
      await loadClients();
    } finally {
      setBusy(null);
    }
  };

  const closeModal = useCallback(() => setModal({ kind: 'none' }), []);

  const handleExportBackup = async (password: string) => {
    if (!soul) return;
    setBusy('Exporting encrypted backup...');
    setError(null);
    try {
      const receipt = await invoke<ExportReceipt>('export_soul_cmd', {
        soulId: soul.soul_id,
        password,
      });
      setSuccess(
        `Backup saved (${receipt.entity_count} entities, ${receipt.event_count} events, ${formatSize(receipt.size_bytes)}). Hash: ${shortHash(receipt.content_hash)}`,
      );
      closeModal();
    } catch {
      setError(safeErrorMessage('сохранить резервную копию'));
    } finally {
      setBusy(null);
    }
  };

  const handleExportJson = async () => {
    if (!soul) return;
    setBusy('Exporting JSON...');
    setError(null);
    try {
      const receipt = await invoke<FileReceipt>('export_soul_json_cmd', {
        soulId: soul.soul_id,
      });
      setSuccess(`JSON export saved (${formatSize(receipt.size_bytes)}).`);
    } catch {
      setError(safeErrorMessage('экспортировать JSON'));
    } finally {
      setBusy(null);
    }
  };

  const handleExportMarkdown = async () => {
    if (!soul) return;
    setBusy('Exporting Markdown...');
    setError(null);
    try {
      const receipt = await invoke<FileReceipt>('export_soul_markdown_cmd', {
        soulId: soul.soul_id,
      });
      setSuccess(`Markdown summary saved (${formatSize(receipt.size_bytes)}).`);
    } catch {
      setError(safeErrorMessage('экспортировать Markdown'));
    } finally {
      setBusy(null);
    }
  };

  const handleRestoreInspect = async (password: string) => {
    setBusy('Verifying backup...');
    setError(null);
    try {
      const selection = await invoke<{ token: string; preview: ImportPreview }>(
        'inspect_soul_file_cmd',
        {
          password,
        },
      );
      setModal({
        kind: 'restore-preview',
        token: selection.token,
        password,
        preview: selection.preview,
      });
    } catch {
      setError(safeErrorMessage('проверить резервную копию'));
    } finally {
      setBusy(null);
    }
  };

  const handleRestoreApply = async (token: string, password: string) => {
    setBusy('Restoring...');
    setError(null);
    try {
      const restored = await invoke<SoulInfo>('import_soul_file_cmd', { token, password });
      setModal({ kind: 'restore-done', soulId: restored.soul_id });
      setSuccess('SOUL restored from backup.');
      await onDataChanged();
    } catch {
      setError(safeErrorMessage('восстановить резервную копию'));
    } finally {
      setBusy(null);
    }
  };

  const handleDelete = async () => {
    if (!soul) return;
    setBusy('Deleting local data...');
    setError(null);
    try {
      const receipt = await invoke<DeletionReceipt>('delete_soul_cmd', { soulId: soul.soul_id });
      localStorage.removeItem(B1_PROFILE_STORAGE_KEY);
      setModal({ kind: 'delete-receipt', receipt });
      await onDataChanged();
    } catch {
      setError(safeErrorMessage('удалить локальные данные'));
    } finally {
      setBusy(null);
    }
  };

  const activeCount = entities.filter((e) => e.status === 'active').length;
  const candidateCount = entities.filter((e) => e.status === 'candidate').length;
  const rejectedCount = entities.filter((e) => e.status === 'rejected').length;

  return (
    <div>
      <h2 style={{ margin: 0 }}>Settings</h2>
      <p style={{ color: '#666' }}>Status, backup, restore and data deletion.</p>

      {error && (
        <div
          role="alert"
          style={{
            ...errorStyle,
            ...(modal.kind !== 'none' ? { display: 'none' } : {}),
          }}
        >
          {error}
          <button onClick={() => setError(null)} style={dismissBtnStyle}>
            x
          </button>
        </div>
      )}
      {success && (
        <div style={successStyle}>
          {success}
          <button onClick={() => setSuccess(null)} style={dismissBtnStyle}>
            x
          </button>
        </div>
      )}

      <Section title="SOUL status">
        <StatusRow label="Display name" value={soul ? soul.display_name : '—'} />
        <StatusRow label="Soul ID" value={soul ? soul.soul_id : '—'} mono />
        <StatusRow label="Device ID" value={soul ? soul.device_id : '—'} mono />
        <StatusRow label="Schema version" value={soul ? soul.schema_version : '—'} />
        <StatusRow label="Format version" value={soul ? soul.format_version : '—'} />
        <StatusRow
          label="Created"
          value={soul ? new Date(soul.created_at).toLocaleString() : '—'}
        />
        <StatusRow
          label="Head event hash"
          value={soul?.head_event_hash ? shortHash(soul.head_event_hash) : '—'}
          mono
        />
        <StatusRow
          label="Entities"
          value={
            soul
              ? `${activeCount} active, ${candidateCount} candidate, ${rejectedCount} rejected`
              : '—'
          }
        />
      </Section>

      <Section title="Backup">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Save an encrypted <code>.soul</code> backup. The file is protected with your passphrase
          and signed by this installation. A save dialog opens after you enter the passphrase. The
          backup includes entities, evaluations, policies, connectors and audit receipts. One-time
          Gateway capabilities are deliberately revoked on restore.
        </p>
        <button
          onClick={() => {
            setError(null);
            setModal({ kind: 'export-passphrase' });
          }}
          disabled={!soul || busy !== null}
          style={primaryBtnStyle}
        >
          Save encrypted backup (.soul)
        </button>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '8px' }}>
          <button
            onClick={handleExportJson}
            disabled={!soul || busy !== null}
            style={secondaryBtnStyle}
          >
            Export JSON
          </button>
          <button
            onClick={handleExportMarkdown}
            disabled={!soul || busy !== null}
            style={secondaryBtnStyle}
          >
            Export Markdown
          </button>
        </div>
      </Section>

      <Section title="Restore">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Choose a <code>.soul</code> backup after entering its passphrase. SOUL verifies the
          package signature, content hash, format and data before anything changes. Restoring
          replaces the current database content only after you confirm the preview.
        </p>
        <button
          onClick={() => {
            setError(null);
            setModal({ kind: 'restore-privacy' });
          }}
          disabled={busy !== null}
          style={secondaryBtnStyle}
        >
          Restore from backup
        </button>
      </Section>

      <Section title="AI clients">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Connect the local MCP server to a supported coding client. The client config is backed up
          before any change, and every operation verifies the result.
        </p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {CLIENT_IDS.map((id) => {
            const status = clients.find((c) => c.client === id);
            if (!status) {
              return (
                <div
                  key={id}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    gap: '12px',
                    alignItems: 'center',
                    padding: '8px 10px',
                    border: '1px solid #e5e7eb',
                    borderRadius: '6px',
                    fontSize: '13px',
                  }}
                >
                  <span>{id}</span>
                  <span style={{ color: '#9ca3af' }}>…</span>
                </div>
              );
            }
            return (
              <div
                key={id}
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '4px 12px',
                  alignItems: 'center',
                  justifyContent: 'space-between',
                  padding: '8px 10px',
                  border: '1px solid #e5e7eb',
                  borderRadius: '6px',
                  fontSize: '13px',
                }}
              >
                <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                  <div>
                    <span style={{ fontWeight: 600 }}>{status.label}</span>{' '}
                    <span style={{ color: '#666' }}>{clientStateLabel(status)}</span>
                  </div>
                  <span style={{ fontSize: '12px', color: '#9ca3af', wordBreak: 'break-all' }}>
                    {clientStatusNote(status)}
                  </span>
                </div>
                <button
                  onClick={() => handleConnect(id)}
                  disabled={busy !== null || !!status.error}
                  style={secondaryBtnStyle}
                >
                  Connect
                </button>
                <button
                  onClick={() => handleDisconnect(id)}
                  disabled={busy !== null || !status.connected}
                  style={secondaryBtnStyle}
                >
                  Disconnect
                </button>
                <button
                  onClick={() => handleRollback(id)}
                  disabled={busy !== null || !status.backup_path}
                  style={secondaryBtnStyle}
                >
                  Rollback
                </button>
              </div>
            );
          })}
        </div>
      </Section>

      <Section title="Browser Companion">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Добавляет разрешённый контекст SOUL в веб-чаты {COMPANION_SITES.join(', ')} без ручного
          копирования: контекст вставляется в то же сообщение и сворачивается в истории в чип{' '}
          <code>SOUL context</code>. Контекст не сохраняется расширением, логами или отчётами об
          ошибках; при изменении разметки сайта расширение отключается само (fail-closed).
        </p>
        <StatusRow label="Host" value={bridgeStateLabel(bridge)} />
        <StatusRow
          label="Notes"
          value={bridge ? bridgeStatusNote(bridge) : 'Нажмите «Проверить».'}
        />
        {bridge?.binary_path ? <StatusRow label="Binary" value={bridge.binary_path} mono /> : null}
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', marginTop: '8px' }}>
          <button onClick={handleBridgeRegister} disabled={busy !== null} style={secondaryBtnStyle}>
            {bridge?.registered ? 'Re-register' : 'Register'}
          </button>
          <button
            onClick={handleBridgeUnregister}
            disabled={busy !== null}
            style={secondaryBtnStyle}
          >
            Unregister
          </button>
          <button
            onClick={() => void loadBridge()}
            disabled={busy !== null}
            style={secondaryBtnStyle}
          >
            Check
          </button>
        </div>
        <p style={{ fontSize: '12px', color: '#9ca3af', margin: '8px 0 0' }}>
          Установите расширение из <code>browser/extension</code> через{' '}
          <code>chrome://extensions</code> (режим разработчика) или загрузите его в свой браузер.
          После отключения host-а веб-чаты работают как обычно — расширение не изменяет их
          поведение.
        </p>
      </Section>

      <Section title="Local receipts">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Deletion and disclosure receipts stored on this device. Receipts contain no personal
          content — only what happened, when and how much.
        </p>
        {usage && usage.disclosure_calls > 0 && (
          <div
            style={{
              fontSize: '12px',
              color: '#4b5563',
              padding: '8px 10px',
              border: '1px solid #e5e7eb',
              borderRadius: '6px',
              marginBottom: '8px',
            }}
          >
            Context disclosures: {usage.disclosure_calls} · {usage.input_tokens_total} input tokens
            total · ~${usage.cost_estimate_usd_total.toFixed(4)} est. cost
            {usage.last_disclosed_at
              ? ` · last ${new Date(usage.last_disclosed_at).toLocaleString()}`
              : ''}
          </div>
        )}
        {receipts.length === 0 ? (
          <p style={{ fontSize: '13px', color: '#9ca3af', margin: 0 }}>No receipts yet.</p>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
            {receipts.map((r) => (
              <div
                key={r.file}
                style={{
                  display: 'flex',
                  flexWrap: 'wrap',
                  gap: '4px 12px',
                  padding: '8px 10px',
                  border: '1px solid #e5e7eb',
                  borderRadius: '6px',
                  fontSize: '12px',
                  color: '#4b5563',
                }}
              >
                <span style={{ fontWeight: 600 }}>{new Date(r.at).toLocaleString()}</span>
                {r.kind === 'deletion' ? (
                  <>
                    <span>
                      {r.entity_count} entities removed, {r.event_count ?? 0} events removed
                    </span>
                    <span>keys deleted: {r.keys_deleted ? 'yes' : 'no'}</span>
                  </>
                ) : (
                  <>
                    <span>
                      context disclosed to <strong>{r.client ?? 'unknown'}</strong> (
                      {r.entity_count} entities)
                    </span>
                    <span>~{r.token_estimate ?? 0} tokens</span>
                    <span>~${(r.cost_estimate_usd ?? 0).toFixed(4)} est.</span>
                    <span>state {r.state_version ?? '—'}</span>
                    <span>policy {r.policy_version ?? '—'}</span>
                  </>
                )}
                <span style={{ color: '#9ca3af', wordBreak: 'break-all' }}>{r.file}</span>
              </div>
            ))}
          </div>
        )}
      </Section>

      <Section title="Danger zone" danger>
        <p style={{ fontSize: '13px', color: '#b91c1c', margin: '0 0 8px' }}>
          Deletes every SOUL and all entities, events, calibration data, policies, gateway records
          and blind-test rounds from the local database. The installation identity and database key
          are retained, and a local deletion receipt is written. Saved backups are not deleted.
        </p>
        <button
          onClick={() => {
            setError(null);
            setModal({ kind: 'delete-confirm' });
          }}
          disabled={!soul || busy !== null}
          style={dangerBtnStyle}
        >
          Delete all local data
        </button>
      </Section>

      {busy && <div style={{ marginTop: '12px', fontSize: '13px', color: '#666' }}>{busy}</div>}

      {modal.kind === 'export-passphrase' && (
        <PassphraseModal
          title="Protect backup with a passphrase"
          confirmLabel="Export"
          busy={busy !== null}
          requireConfirmation
          error={error}
          onSubmit={(password) => handleExportBackup(password)}
          onClose={closeModal}
        />
      )}

      {modal.kind === 'restore-privacy' && (
        <Modal title="Перед восстановлением" onClose={closeModal}>
          <p style={{ fontSize: '13px', color: '#4b5563', lineHeight: 1.5 }}>
            Файл и пароль обрабатываются локально. Ничего не отправляется в сеть. Сначала SOUL
            проверит подпись, hash и схему, затем покажет preview. Локальные данные заменяются
            только после отдельного подтверждения.
          </p>
          <div
            style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}
          >
            <button onClick={closeModal} style={secondaryBtnStyle}>
              Cancel
            </button>
            <button
              onClick={() => {
                closeModal();
                setModal({ kind: 'restore-passphrase' });
              }}
              style={primaryBtnStyle}
            >
              Continue
            </button>
          </div>
        </Modal>
      )}

      {modal.kind === 'restore-passphrase' && (
        <PassphraseModal
          title="Enter backup passphrase"
          confirmLabel="Choose and verify backup"
          busy={busy !== null}
          error={error}
          onSubmit={handleRestoreInspect}
          onClose={closeModal}
        />
      )}

      {modal.kind === 'restore-preview' && (
        <Modal
          title="Restore preview"
          onClose={closeModal}
          closeOnBackdrop={false}
          closeOnEscape={busy === null}
          ariaDescribedBy={
            error
              ? 'restore-preview-description restore-preview-error'
              : 'restore-preview-description'
          }
        >
          <p
            id="restore-preview-description"
            style={{
              fontSize: '13px',
              color: '#b91c1c',
              background: '#fef2f2',
              padding: '8px 12px',
              borderRadius: '6px',
              border: '1px solid #fecaca',
            }}
          >
            Restoring replaces the current local SOUL data.
          </p>
          {modal.preview.partial_restore && (
            <p
              style={{
                fontSize: '13px',
                color: '#92400e',
                background: '#fffbeb',
                padding: '8px 12px',
                borderRadius: '6px',
                border: '1px solid #fde68a',
              }}
            >
              This is a legacy core-only backup. Evaluations, policies, connector settings and
              Gateway receipts were not included, so local demo defaults will be recreated.
            </p>
          )}
          {error && (
            <p id="restore-preview-error" role="alert" style={modalErrorStyle}>
              {error}
            </p>
          )}
          <div style={{ fontSize: '13px', lineHeight: 1.7 }}>
            <PreviewRow label="Name" value={modal.preview.display_name} />
            <PreviewRow label="Soul ID" value={modal.preview.soul_id} mono />
            <PreviewRow
              label="Created"
              value={new Date(modal.preview.created_at).toLocaleString()}
            />
            <PreviewRow label="Entities" value={String(modal.preview.entity_count)} />
            <PreviewRow label="Events" value={String(modal.preview.event_count)} />
            <PreviewRow label="Calibration step" value={String(modal.preview.calibration_step)} />
            <PreviewRow label="Activated" value={modal.preview.activated ? 'yes' : 'no'} />
            {modal.preview.head_event_hash && (
              <PreviewRow
                label="Head event hash"
                value={shortHash(modal.preview.head_event_hash)}
                mono
              />
            )}
            {modal.preview.entity_counts.map((c) => (
              <PreviewRow key={c.entity_type} label={c.entity_type} value={String(c.count)} />
            ))}
          </div>
          <div
            style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}
          >
            <button onClick={closeModal} disabled={busy !== null} style={secondaryBtnStyle}>
              Cancel
            </button>
            <button
              onClick={() => handleRestoreApply(modal.token, modal.password)}
              disabled={busy !== null}
              style={primaryBtnStyle}
            >
              {busy ? 'Restoring...' : 'Confirm restore'}
            </button>
          </div>
        </Modal>
      )}

      {modal.kind === 'restore-done' && (
        <Modal title="Restore complete" onClose={closeModal}>
          <p style={{ fontSize: '13px', color: '#666' }}>
            The backup has been restored. Your SOUL is ready.
          </p>
          <div style={{ marginTop: '16px', display: 'flex', justifyContent: 'flex-end' }}>
            <button
              onClick={() => {
                closeModal();
                onGoHome();
              }}
              style={primaryBtnStyle}
            >
              Go to Home
            </button>
          </div>
        </Modal>
      )}

      {modal.kind === 'delete-confirm' && (
        <DeleteConfirmModal
          busy={busy !== null}
          error={error}
          onConfirm={handleDelete}
          onClose={closeModal}
        />
      )}

      {modal.kind === 'delete-receipt' && (
        <Modal title="Data deleted" onClose={closeModal}>
          <p style={{ fontSize: '13px', color: '#666', margin: '0 0 12px' }}>
            The SOUL database content was removed. The installation identity, database key and this
            deletion receipt remain locally.
          </p>
          <div style={{ fontSize: '13px', lineHeight: 1.7 }}>
            <PreviewRow
              label="Deleted at"
              value={new Date(modal.receipt.deleted_at).toLocaleString()}
            />
            <PreviewRow label="Entities removed" value={String(modal.receipt.entity_count)} />
            <PreviewRow label="Events removed" value={String(modal.receipt.event_count)} />
            <PreviewRow
              label="Installation keys removed"
              value={modal.receipt.keys_deleted ? 'yes' : 'no'}
            />
          </div>
          <div style={{ marginTop: '16px', display: 'flex', justifyContent: 'flex-end' }}>
            <button
              onClick={() => {
                closeModal();
                onGoHome();
              }}
              style={primaryBtnStyle}
            >
              Done
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}

function Section({
  title,
  children,
  danger,
}: {
  title: string;
  children: React.ReactNode;
  danger?: boolean;
}) {
  return (
    <div
      style={{
        marginTop: '24px',
        padding: '16px',
        border: danger ? '1px solid #fecaca' : '1px solid #e5e7eb',
        borderRadius: '10px',
        background: danger ? '#fffbfb' : '#fff',
      }}
    >
      <h3 style={{ margin: '0 0 8px', fontSize: '15px', color: danger ? '#b91c1c' : '#111' }}>
        {title}
      </h3>
      {children}
    </div>
  );
}

function StatusRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div
      className="status-row"
      style={{
        display: 'flex',
        justifyContent: 'space-between',
        gap: '16px',
        padding: '4px 0',
        fontSize: '13px',
      }}
    >
      <span style={{ color: '#888' }}>{label}</span>
      <span
        style={{
          fontFamily: mono ? 'Consolas, monospace' : 'inherit',
          wordBreak: 'break-all',
          textAlign: 'right',
        }}
      >
        {value}
      </span>
    </div>
  );
}

function PreviewRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div
      className="status-row"
      style={{ display: 'flex', justifyContent: 'space-between', gap: '16px' }}
    >
      <span style={{ color: '#888' }}>{label}</span>
      <span
        style={{
          fontFamily: mono ? 'Consolas, monospace' : 'inherit',
          wordBreak: 'break-all',
          textAlign: 'right',
        }}
      >
        {value}
      </span>
    </div>
  );
}

function PassphraseModal({
  title,
  confirmLabel,
  busy,
  error,
  requireConfirmation,
  onSubmit,
  onClose,
}: {
  title: string;
  confirmLabel: string;
  busy: boolean;
  error: string | null;
  requireConfirmation?: boolean;
  onSubmit: (password: string) => void;
  onClose: () => void;
}) {
  const [pass, setPass] = useState('');
  const [pass2, setPass2] = useState('');
  const [localError, setLocalError] = useState<string | null>(null);

  const submit = () => {
    if (pass.length < 8) {
      setLocalError('Passphrase must be at least 8 characters.');
      return;
    }
    if (requireConfirmation && pass !== pass2) {
      setLocalError('Passphrases do not match.');
      return;
    }
    setLocalError(null);
    onSubmit(pass);
  };

  return (
    <Modal
      title={title}
      onClose={onClose}
      closeOnBackdrop={!busy}
      closeOnEscape={!busy}
      {...(localError || error ? { ariaDescribedBy: 'passphrase-modal-error' } : {})}
    >
      <label htmlFor="modal-passphrase" style={fieldLabelStyle}>
        Passphrase
      </label>
      <input
        id="modal-passphrase"
        type="password"
        value={pass}
        onChange={(e) => setPass(e.target.value)}
        autoComplete={requireConfirmation ? 'new-password' : 'current-password'}
        autoFocus
        style={inputStyle}
      />
      {requireConfirmation && (
        <>
          <label
            htmlFor="modal-passphrase-confirm"
            style={{ ...fieldLabelStyle, marginTop: '8px' }}
          >
            Confirm passphrase
          </label>
          <input
            id="modal-passphrase-confirm"
            type="password"
            value={pass2}
            onChange={(e) => setPass2(e.target.value)}
            autoComplete="new-password"
            style={{ ...inputStyle, marginTop: '8px' }}
          />
        </>
      )}
      {(localError || error) && (
        <p id="passphrase-modal-error" role="alert" style={modalErrorStyle}>
          {localError ?? error}
        </p>
      )}
      <div style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
        <button onClick={onClose} disabled={busy} style={secondaryBtnStyle}>
          Cancel
        </button>
        <button onClick={submit} disabled={busy} style={primaryBtnStyle}>
          {busy ? 'Working...' : confirmLabel}
        </button>
      </div>
    </Modal>
  );
}

function DeleteConfirmModal({
  busy,
  error,
  onConfirm,
  onClose,
}: {
  busy: boolean;
  error: string | null;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const [typed, setTyped] = useState('');
  const [localError, setLocalError] = useState<string | null>(null);

  const submit = () => {
    if (typed.trim().toUpperCase() !== 'DELETE') {
      setLocalError('Type DELETE to confirm.');
      return;
    }
    setLocalError(null);
    onConfirm();
  };

  return (
    <Modal
      title="Delete all local data?"
      titleStyle={{ color: '#b91c1c' }}
      onClose={onClose}
      closeOnBackdrop={!busy}
      closeOnEscape={!busy}
      ariaDescribedBy={
        localError || error
          ? 'delete-modal-description delete-modal-error'
          : 'delete-modal-description'
      }
    >
      <p
        id="delete-modal-description"
        style={{ fontSize: '13px', color: '#666', margin: '0 0 12px' }}
      >
        This permanently deletes every SOUL and all entities, events, calibration data, policies,
        gateway records and blind-test rounds in the local database. The installation identity and
        database key remain, and a deletion receipt is written locally. This cannot be undone.
      </p>
      <label htmlFor="delete-confirmation" style={fieldLabelStyle}>
        Type DELETE to confirm
      </label>
      <input
        id="delete-confirmation"
        type="text"
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        autoComplete="off"
        autoFocus
        style={inputStyle}
      />
      {(localError || error) && (
        <p id="delete-modal-error" role="alert" style={modalErrorStyle}>
          {localError ?? error}
        </p>
      )}
      <div style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
        <button onClick={onClose} disabled={busy} style={secondaryBtnStyle}>
          Cancel
        </button>
        <button onClick={submit} disabled={busy} style={dangerBtnStyle}>
          {busy ? 'Deleting...' : 'Delete everything'}
        </button>
      </div>
    </Modal>
  );
}

function shortHash(h: string): string {
  return h.length > 16 ? `${h.slice(0, 8)}…${h.slice(-8)}` : h;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

const inputStyle: React.CSSProperties = {
  width: '100%',
  padding: '8px 12px',
  border: '1px solid #d1d5db',
  borderRadius: '6px',
  boxSizing: 'border-box',
};

const fieldLabelStyle: React.CSSProperties = {
  display: 'block',
  marginBottom: '4px',
  color: '#374151',
  fontSize: '13px',
  fontWeight: 600,
};

const modalErrorStyle: React.CSSProperties = {
  color: '#b91c1c',
  fontSize: '12px',
  margin: '8px 0 0',
};

const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 20px',
  background: '#6366f1',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontWeight: 600,
};

const secondaryBtnStyle: React.CSSProperties = {
  padding: '8px 20px',
  background: '#f3f4f6',
  color: '#333',
  border: '1px solid #d1d5db',
  borderRadius: '6px',
  cursor: 'pointer',
};

const dangerBtnStyle: React.CSSProperties = {
  padding: '8px 20px',
  background: '#dc2626',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontWeight: 600,
};

const errorStyle: React.CSSProperties = {
  padding: '8px 12px',
  background: '#fef2f2',
  border: '1px solid #fecaca',
  borderRadius: '6px',
  margin: '12px 0',
  color: '#dc2626',
  fontSize: '13px',
};

const successStyle: React.CSSProperties = {
  padding: '8px 12px',
  background: '#f0fdf4',
  border: '1px solid #bbf7d0',
  borderRadius: '6px',
  margin: '12px 0',
  color: '#16a34a',
  fontSize: '13px',
};

const dismissBtnStyle: React.CSSProperties = {
  marginLeft: '8px',
  background: 'none',
  border: 'none',
  cursor: 'pointer',
  color: 'inherit',
};
