import { useState } from 'react';
import { open, save } from '@tauri-apps/plugin-dialog';

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
}

interface DeletionReceipt {
  deleted_at: string;
  entity_count: number;
  event_count: number;
  keys_deleted: boolean;
}

type ModalState =
  | { kind: 'none' }
  | { kind: 'export-passphrase' }
  | { kind: 'restore-passphrase'; filePath: string }
  | { kind: 'restore-preview'; filePath: string; password: string; preview: ImportPreview }
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

function ensureExtension(path: string, ext: string): string {
  return path.toLowerCase().endsWith(ext) ? path : `${path}${ext}`;
}

function sanitizeFileName(name: string): string {
  return name.replace(/[^a-zA-Z0-9-_ ]/g, '').trim() || 'soul';
}

export function Settings({ soul, entities, onDataChanged, onGoHome }: SettingsProps) {
  const [modal, setModal] = useState<ModalState>({ kind: 'none' });
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const closeModal = () => setModal({ kind: 'none' });

  const handleExportBackup = async (password: string) => {
    if (!soul) return;
    setBusy('Exporting encrypted backup...');
    setError(null);
    try {
      const path = await save({
        title: 'Save SOUL backup',
        defaultPath: `${sanitizeFileName(soul.display_name)}.soul`,
        filters: [{ name: 'SOUL backup', extensions: ['soul'] }],
      });
      if (!path) {
        closeModal();
        return;
      }
      const receipt = await invoke<ExportReceipt>('export_soul_cmd', {
        soulId: soul.soul_id,
        password,
        path: ensureExtension(path, '.soul'),
      });
      setSuccess(
        `Backup saved (${receipt.entity_count} entities, ${receipt.event_count} events, ${formatSize(receipt.size_bytes)}). Hash: ${shortHash(receipt.content_hash)}`,
      );
      closeModal();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleExportJson = async () => {
    if (!soul) return;
    setBusy('Exporting JSON...');
    setError(null);
    try {
      const path = await save({
        title: 'Export SOUL as JSON',
        defaultPath: `${sanitizeFileName(soul.display_name)}-export.json`,
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!path) return;
      const receipt = await invoke<FileReceipt>('export_soul_json_cmd', {
        soulId: soul.soul_id,
        path: ensureExtension(path, '.json'),
      });
      setSuccess(`JSON export saved (${formatSize(receipt.size_bytes)}).`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleExportMarkdown = async () => {
    if (!soul) return;
    setBusy('Exporting Markdown...');
    setError(null);
    try {
      const path = await save({
        title: 'Export SOUL as Markdown',
        defaultPath: `${sanitizeFileName(soul.display_name)}-summary.md`,
        filters: [{ name: 'Markdown', extensions: ['md'] }],
      });
      if (!path) return;
      const receipt = await invoke<FileReceipt>('export_soul_markdown_cmd', {
        soulId: soul.soul_id,
        path: ensureExtension(path, '.md'),
      });
      setSuccess(`Markdown summary saved (${formatSize(receipt.size_bytes)}).`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleRestoreStart = async () => {
    setError(null);
    const filePath = await open({
      title: 'Restore SOUL backup',
      multiple: false,
      directory: false,
      filters: [{ name: 'SOUL backup', extensions: ['soul'] }],
    });
    if (!filePath) return;
    setModal({ kind: 'restore-passphrase', filePath });
  };

  const handleRestoreInspect = async (filePath: string, password: string) => {
    setBusy('Verifying backup...');
    setError(null);
    try {
      const preview = await invoke<ImportPreview>('inspect_soul_file_cmd', { path: filePath, password });
      setModal({ kind: 'restore-preview', filePath, password, preview });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleRestoreApply = async (filePath: string, password: string) => {
    setBusy('Restoring...');
    setError(null);
    try {
      const restored = await invoke<SoulInfo>('import_soul_file_cmd', { path: filePath, password });
      setModal({ kind: 'restore-done', soulId: restored.soul_id });
      setSuccess('SOUL restored from backup.');
      await onDataChanged();
    } catch (e) {
      setError(String(e));
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
      setModal({ kind: 'delete-receipt', receipt });
      await onDataChanged();
    } catch (e) {
      setError(String(e));
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
          style={{
            ...errorStyle,
            ...(modal.kind !== 'none'
              ? { position: 'fixed', top: '16px', left: '50%', transform: 'translateX(-50%)', zIndex: 200, maxWidth: 'min(560px, 90vw)' }
              : {}),
          }}
        >
          {error}
          <button onClick={() => setError(null)} style={dismissBtnStyle}>x</button>
        </div>
      )}
      {success && (
        <div style={successStyle}>
          {success}
          <button onClick={() => setSuccess(null)} style={dismissBtnStyle}>x</button>
        </div>
      )}

      <Section title="SOUL status">
        <StatusRow label="Display name" value={soul ? soul.display_name : '—'} />
        <StatusRow label="Soul ID" value={soul ? soul.soul_id : '—'} mono />
        <StatusRow label="Device ID" value={soul ? soul.device_id : '—'} mono />
        <StatusRow label="Schema version" value={soul ? soul.schema_version : '—'} />
        <StatusRow label="Format version" value={soul ? soul.format_version : '—'} />
        <StatusRow label="Created" value={soul ? new Date(soul.created_at).toLocaleString() : '—'} />
        <StatusRow label="Head event hash" value={soul?.head_event_hash ? shortHash(soul.head_event_hash) : '—'} mono />
        <StatusRow label="Entities" value={soul ? `${activeCount} active, ${candidateCount} candidate, ${rejectedCount} rejected` : '—'} />
      </Section>

      <Section title="Backup">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Save an encrypted <code>.soul</code> backup. The file is protected with your passphrase and signed by this device. Restoring works on any machine that knows the passphrase.
        </p>
        <button
          onClick={() => setModal({ kind: 'export-passphrase' })}
          disabled={!soul || busy !== null}
          style={primaryBtnStyle}
        >
          Save encrypted backup (.soul)
        </button>
        <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
          <button onClick={handleExportJson} disabled={!soul || busy !== null} style={secondaryBtnStyle}>
            Export JSON
          </button>
          <button onClick={handleExportMarkdown} disabled={!soul || busy !== null} style={secondaryBtnStyle}>
            Export Markdown
          </button>
        </div>
      </Section>

      <Section title="Restore">
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 8px' }}>
          Import a <code>.soul</code> backup. The file is verified (signature, hash, schema) before anything changes, and restoring replaces the current local data after you confirm a preview.
        </p>
        <button onClick={handleRestoreStart} disabled={busy !== null} style={secondaryBtnStyle}>
          Restore from backup
        </button>
      </Section>

      <Section title="Danger zone" danger>
        <p style={{ fontSize: '13px', color: '#b91c1c', margin: '0 0 8px' }}>
          Deletes the active SOUL, all entities, events, device keys and local database content. A deletion receipt is written locally. Backups remain wherever you saved them.
        </p>
        <button
          onClick={() => setModal({ kind: 'delete-confirm' })}
          disabled={!soul || busy !== null}
          style={dangerBtnStyle}
        >
          Delete all local data
        </button>
      </Section>

      {busy && (
        <div style={{ marginTop: '12px', fontSize: '13px', color: '#666' }}>{busy}</div>
      )}

      {modal.kind === 'export-passphrase' && (
        <PassphraseModal
          title="Protect backup with a passphrase"
          confirmLabel="Export"
          busy={busy !== null}
          requireConfirmation
          onSubmit={(password) => handleExportBackup(password)}
          onClose={closeModal}
        />
      )}

      {modal.kind === 'restore-passphrase' && (
        <PassphraseModal
          title="Enter backup passphrase"
          confirmLabel="Verify"
          busy={busy !== null}
          onSubmit={(password) => handleRestoreInspect(modal.filePath, password)}
          onClose={closeModal}
        />
      )}

      {modal.kind === 'restore-preview' && (
        <div style={modalBackdropStyle}>
          <div style={modalCardStyle}>
            <h3 style={{ margin: '0 0 12px' }}>Restore preview</h3>
            <p style={{ fontSize: '13px', color: '#b91c1c', background: '#fef2f2', padding: '8px 12px', borderRadius: '6px', border: '1px solid #fecaca' }}>
              Restoring replaces the current local SOUL data.
            </p>
            <div style={{ fontSize: '13px', lineHeight: 1.7 }}>
              <PreviewRow label="Name" value={modal.preview.display_name} />
              <PreviewRow label="Soul ID" value={modal.preview.soul_id} mono />
              <PreviewRow label="Created" value={new Date(modal.preview.created_at).toLocaleString()} />
              <PreviewRow label="Entities" value={String(modal.preview.entity_count)} />
              <PreviewRow label="Events" value={String(modal.preview.event_count)} />
              <PreviewRow label="Calibration step" value={String(modal.preview.calibration_step)} />
              <PreviewRow label="Activated" value={modal.preview.activated ? 'yes' : 'no'} />
              {modal.preview.head_event_hash && (
                <PreviewRow label="Head event hash" value={shortHash(modal.preview.head_event_hash)} mono />
              )}
              {modal.preview.entity_counts.map((c) => (
                <PreviewRow key={c.entity_type} label={c.entity_type} value={String(c.count)} />
              ))}
            </div>
            <div style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button onClick={closeModal} style={secondaryBtnStyle}>Cancel</button>
              <button onClick={() => handleRestoreApply(modal.filePath, modal.password)} disabled={busy !== null} style={primaryBtnStyle}>
                {busy ? 'Restoring...' : 'Confirm restore'}
              </button>
            </div>
          </div>
        </div>
      )}

      {modal.kind === 'restore-done' && (
        <div style={modalBackdropStyle}>
          <div style={modalCardStyle}>
            <h3 style={{ margin: '0 0 8px' }}>Restore complete</h3>
            <p style={{ fontSize: '13px', color: '#666' }}>The backup has been restored. Your SOUL is ready.</p>
            <div style={{ marginTop: '16px', display: 'flex', justifyContent: 'flex-end' }}>
              <button onClick={() => { closeModal(); onGoHome(); }} style={primaryBtnStyle}>Go to Home</button>
            </div>
          </div>
        </div>
      )}

      {modal.kind === 'delete-confirm' && (
        <DeleteConfirmModal
          busy={busy !== null}
          onConfirm={handleDelete}
          onClose={closeModal}
        />
      )}

      {modal.kind === 'delete-receipt' && (
        <div style={modalBackdropStyle}>
          <div style={modalCardStyle}>
            <h3 style={{ margin: '0 0 8px' }}>Data deleted</h3>
            <p style={{ fontSize: '13px', color: '#666', margin: '0 0 12px' }}>
              All local SOUL data was removed. This receipt is stored locally for your reference.
            </p>
            <div style={{ fontSize: '13px', lineHeight: 1.7 }}>
              <PreviewRow label="Deleted at" value={new Date(modal.receipt.deleted_at).toLocaleString()} />
              <PreviewRow label="Entities removed" value={String(modal.receipt.entity_count)} />
              <PreviewRow label="Events removed" value={String(modal.receipt.event_count)} />
              <PreviewRow label="Device keys removed" value={modal.receipt.keys_deleted ? 'yes' : 'no'} />
            </div>
            <div style={{ marginTop: '16px', display: 'flex', justifyContent: 'flex-end' }}>
              <button onClick={() => { closeModal(); onGoHome(); }} style={primaryBtnStyle}>Done</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Section({ title, children, danger }: { title: string; children: React.ReactNode; danger?: boolean }) {
  return (
    <div style={{ marginTop: '24px', padding: '16px', border: danger ? '1px solid #fecaca' : '1px solid #e5e7eb', borderRadius: '10px', background: danger ? '#fffbfb' : '#fff' }}>
      <h3 style={{ margin: '0 0 8px', fontSize: '15px', color: danger ? '#b91c1c' : '#111' }}>{title}</h3>
      {children}
    </div>
  );
}

function StatusRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '16px', padding: '4px 0', fontSize: '13px' }}>
      <span style={{ color: '#888' }}>{label}</span>
      <span style={{ fontFamily: mono ? 'Consolas, monospace' : 'inherit', wordBreak: 'break-all', textAlign: 'right' }}>{value}</span>
    </div>
  );
}

function PreviewRow({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: '16px' }}>
      <span style={{ color: '#888' }}>{label}</span>
      <span style={{ fontFamily: mono ? 'Consolas, monospace' : 'inherit', wordBreak: 'break-all', textAlign: 'right' }}>{value}</span>
    </div>
  );
}

function PassphraseModal({
  title, confirmLabel, busy, requireConfirmation, onSubmit, onClose,
}: {
  title: string;
  confirmLabel: string;
  busy: boolean;
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
    <div style={modalBackdropStyle}>
      <div style={modalCardStyle}>
        <h3 style={{ margin: '0 0 12px' }}>{title}</h3>
        <input
          type="password"
          value={pass}
          onChange={(e) => setPass(e.target.value)}
          placeholder="Passphrase"
          style={inputStyle}
        />
        {requireConfirmation && (
          <input
            type="password"
            value={pass2}
            onChange={(e) => setPass2(e.target.value)}
            placeholder="Confirm passphrase"
            style={{ ...inputStyle, marginTop: '8px' }}
          />
        )}
        {localError && <p style={{ color: '#dc2626', fontSize: '12px', margin: '8px 0 0' }}>{localError}</p>}
        <div style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
          <button onClick={onClose} disabled={busy} style={secondaryBtnStyle}>Cancel</button>
          <button onClick={submit} disabled={busy} style={primaryBtnStyle}>
            {busy ? 'Working...' : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

function DeleteConfirmModal({
  busy, onConfirm, onClose,
}: {
  busy: boolean;
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
    <div style={modalBackdropStyle}>
      <div style={modalCardStyle}>
        <h3 style={{ margin: '0 0 12px', color: '#b91c1c' }}>Delete all local data?</h3>
        <p style={{ fontSize: '13px', color: '#666', margin: '0 0 12px' }}>
          This permanently deletes your SOUL, entities, events, calibration and device keys on this machine. This cannot be undone.
        </p>
        <input
          type="text"
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          placeholder="Type DELETE"
          style={inputStyle}
        />
        {localError && <p style={{ color: '#dc2626', fontSize: '12px', margin: '8px 0 0' }}>{localError}</p>}
        <div style={{ marginTop: '16px', display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
          <button onClick={onClose} disabled={busy} style={secondaryBtnStyle}>Cancel</button>
          <button onClick={submit} disabled={busy} style={dangerBtnStyle}>
            {busy ? 'Deleting...' : 'Delete everything'}
          </button>
        </div>
      </div>
    </div>
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

const modalBackdropStyle: React.CSSProperties = {
  position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.4)',
  display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 100,
};

const modalCardStyle: React.CSSProperties = {
  background: '#fff', borderRadius: '12px', padding: '20px',
  width: 'min(480px, 90vw)', boxShadow: '0 10px 40px rgba(0,0,0,0.2)',
};

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '8px 12px', border: '1px solid #d1d5db', borderRadius: '6px', boxSizing: 'border-box',
};

const primaryBtnStyle: React.CSSProperties = {
  padding: '8px 20px', background: '#6366f1', color: '#fff',
  border: 'none', borderRadius: '6px', cursor: 'pointer', fontWeight: 600,
};

const secondaryBtnStyle: React.CSSProperties = {
  padding: '8px 20px', background: '#f3f4f6', color: '#333',
  border: '1px solid #d1d5db', borderRadius: '6px', cursor: 'pointer',
};

const dangerBtnStyle: React.CSSProperties = {
  padding: '8px 20px', background: '#dc2626', color: '#fff',
  border: 'none', borderRadius: '6px', cursor: 'pointer', fontWeight: 600,
};

const errorStyle: React.CSSProperties = {
  padding: '8px 12px', background: '#fef2f2', border: '1px solid #fecaca',
  borderRadius: '6px', margin: '12px 0', color: '#dc2626', fontSize: '13px',
};

const successStyle: React.CSSProperties = {
  padding: '8px 12px', background: '#f0fdf4', border: '1px solid #bbf7d0',
  borderRadius: '6px', margin: '12px 0', color: '#16a34a', fontSize: '13px',
};

const dismissBtnStyle: React.CSSProperties = {
  marginLeft: '8px', background: 'none', border: 'none', cursor: 'pointer', color: 'inherit',
};
