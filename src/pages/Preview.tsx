import { useState } from 'react';
import {
  claimOf,
  formatSourceDate,
  maskText,
  parseEntityData,
  rankCandidates,
  requiresExplicitConfirm,
  type ReviewEntity,
} from '../data/review';

interface PreviewProps {
  entities: ReviewEntity[];
  previewConfirmed: boolean;
  busyId: string | null;
  onEdit: (id: string, claim: string) => void;
  onConfirmPreview: () => void;
  onActivate: (entityIds: string[]) => void;
  onBack: () => void;
}

export function Preview({
  entities,
  previewConfirmed,
  busyId,
  onEdit,
  onConfirmPreview,
  onActivate,
  onBack,
}: PreviewProps) {
  const candidates = rankCandidates(entities.filter((e) => e.status === 'candidate'));
  const [included, setIncluded] = useState<Record<string, boolean>>({});
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');

  const isIncluded = (e: ReviewEntity): boolean =>
    !requiresExplicitConfirm(e) && (included[e.id] ?? true);

  const includedCount = candidates.filter((e) => isIncluded(e)).length;
  const individualCount = candidates.filter((e) => requiresExplicitConfirm(e)).length;

  const handleEditSave = (id: string) => {
    const target = candidates.find((e) => e.id === id);
    if (!target) return;
    const trimmed = draft.trim();
    if (trimmed.length === 0 || trimmed === claimOf(target)) {
      setEditingId(null);
      return;
    }
    onEdit(id, trimmed);
    setEditingId(null);
  };

  return (
    <div>
      <h2 style={{ margin: '0 0 4px' }}>Preview your SOUL</h2>
      <p style={{ color: '#888', fontSize: '14px', margin: '0 0 12px' }}>
        What SOUL learned from your calibration. Exclude anything you don't want, then confirm and
        activate.
      </p>

      <div style={{ display: 'flex', gap: '16px', flexWrap: 'wrap', marginBottom: '16px' }}>
        <Chip label="Candidates" value={candidates.length} />
        <Chip label="Will be activated" value={includedCount} accent />
        {individualCount > 0 && (
          <Chip label="Need individual review" value={individualCount} warn />
        )}
      </div>

      {candidates.length === 0 ? (
        <div
          style={{
            padding: '24px',
            border: '1px dashed #d1d5db',
            borderRadius: '8px',
            textAlign: 'center',
            color: '#888',
            marginBottom: '16px',
          }}
        >
          No new items from calibration. You can still confirm and activate.
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', marginBottom: '16px' }}>
          {candidates.map((e) => {
            const data = parseEntityData(e.data);
            const locked = requiresExplicitConfirm(e);
            const checked = isIncluded(e);
            return (
              <div
                key={e.id}
                style={{
                  display: 'flex',
                  gap: '10px',
                  alignItems: 'start',
                  padding: '10px 12px',
                  border: locked ? '1px solid #fcd34d' : '1px solid #e5e7eb',
                  borderRadius: '8px',
                  background: locked ? '#fffbeb' : '#fff',
                }}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={locked || busyId === e.id}
                  onChange={(ev) => setIncluded((prev) => ({ ...prev, [e.id]: ev.target.checked }))}
                  style={{ marginTop: '16px', width: '16px', height: '16px' }}
                  aria-label={`Include ${e.entity_type} item`}
                />
                <div style={{ minWidth: '0', flex: '1' }}>
                  <div
                    style={{ display: 'flex', gap: '6px', flexWrap: 'wrap', alignItems: 'center' }}
                  >
                    <span style={badgeStyle}>{e.entity_type}</span>
                    {data.disputed === true && <span style={disputedBadgeStyle}>disputed</span>}
                    {locked && (
                      <span style={{ fontSize: '12px', color: '#b45309' }}>
                        requires individual confirmation in Inbox
                      </span>
                    )}
                  </div>
                  {editingId === e.id ? (
                    <div style={{ marginTop: '6px' }}>
                      <textarea
                        value={draft}
                        onChange={(ev) => setDraft(ev.target.value)}
                        style={{
                          width: '100%',
                          minHeight: '52px',
                          padding: '8px',
                          border: '1px solid #d1d5db',
                          borderRadius: '6px',
                          resize: 'vertical',
                          fontSize: '14px',
                        }}
                        aria-label="Edit statement"
                      />
                      <div style={{ display: 'flex', gap: '6px', marginTop: '6px' }}>
                        <button onClick={() => handleEditSave(e.id)} style={saveBtnStyle}>
                          Save
                        </button>
                        <button onClick={() => setEditingId(null)} style={cancelBtnStyle}>
                          Cancel
                        </button>
                      </div>
                    </div>
                  ) : (
                    <>
                      <p
                        style={{
                          margin: '6px 0 4px',
                          fontWeight: 500,
                          fontSize: '14px',
                          overflowWrap: 'anywhere',
                        }}
                      >
                        {maskText(claimOf(e))}
                      </p>
                      <div
                        style={{
                          display: 'flex',
                          flexWrap: 'wrap',
                          gap: '8px',
                          fontSize: '12px',
                          color: '#888',
                        }}
                      >
                        <span>
                          Confidence:{' '}
                          {data.confidence !== undefined
                            ? `${Math.round(data.confidence * 100)}%`
                            : '—'}
                        </span>
                        <span>Sensitivity: {data.sensitivity ?? '—'}</span>
                        <span>Source: {formatSourceDate(e.created_at)}</span>
                        {data.scope && data.scope.domains.length > 0 && (
                          <span>Scope: {data.scope.domains.join(', ')}</span>
                        )}
                      </div>
                    </>
                  )}
                </div>
                <button
                  onClick={() => {
                    setEditingId(e.id);
                    setDraft(claimOf(e));
                  }}
                  disabled={busyId === e.id}
                  style={editBtnStyle}
                >
                  Edit
                </button>
              </div>
            );
          })}
        </div>
      )}

      <div
        style={{
          position: 'sticky',
          bottom: '8px',
          padding: '14px 16px',
          background: '#fff',
          border: '1px solid #e5e7eb',
          borderRadius: '10px',
          boxShadow: '0 4px 12px rgba(0,0,0,0.06)',
        }}
      >
        {!previewConfirmed ? (
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center', flexWrap: 'wrap' }}>
            <p style={{ margin: 0, fontSize: '13px', color: '#4b5563', flex: '1 1 280px' }}>
              Activation happens only after you explicitly confirm this preview. Sensitive and
              boundary items always stay in Inbox for individual confirmation.
            </p>
            <button onClick={onConfirmPreview} style={confirmBtnStyle}>
              I've reviewed the preview — confirm it
            </button>
            <button onClick={onBack} style={cancelBtnStyle}>
              Back
            </button>
          </div>
        ) : (
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center', flexWrap: 'wrap' }}>
            <span style={{ fontSize: '13px', color: '#16a34a', fontWeight: 600 }}>
              Preview confirmed ✓
            </span>
            <p style={{ margin: 0, fontSize: '13px', color: '#6b7280', flex: '1 1 240px' }}>
              Activating {includedCount} item{includedCount !== 1 ? 's' : ''}. Everything excluded
              stays as a candidate in Inbox.
            </p>
            <button
              onClick={() => onActivate(candidates.filter((e) => isIncluded(e)).map((e) => e.id))}
              disabled={busyId !== null}
              style={{ ...activateBtnStyle, opacity: busyId !== null ? 0.5 : 1 }}
            >
              {busyId !== null ? 'Working...' : `Activate SOUL (${includedCount})`}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function Chip({
  label,
  value,
  accent,
  warn,
}: {
  label: string;
  value: number;
  accent?: boolean;
  warn?: boolean;
}) {
  return (
    <div style={{ textAlign: 'center' }}>
      <div
        style={{
          fontSize: '20px',
          fontWeight: 700,
          color: accent ? '#22c55e' : warn ? '#b45309' : '#6366f1',
        }}
      >
        {value}
      </div>
      <div style={{ fontSize: '12px', color: '#888' }}>{label}</div>
    </div>
  );
}

const badgeStyle: React.CSSProperties = {
  fontSize: '11px',
  padding: '2px 8px',
  borderRadius: '4px',
  background: '#eef2ff',
  color: '#6366f1',
  fontWeight: 600,
  textTransform: 'uppercase',
};

const disputedBadgeStyle: React.CSSProperties = {
  fontSize: '11px',
  padding: '2px 8px',
  borderRadius: '4px',
  background: '#fef3c7',
  color: '#b45309',
  fontWeight: 700,
  textTransform: 'uppercase',
};

const confirmBtnStyle: React.CSSProperties = {
  padding: '8px 18px',
  background: '#6366f1',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontWeight: 600,
  fontSize: '14px',
};

const activateBtnStyle: React.CSSProperties = {
  padding: '8px 18px',
  background: '#22c55e',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontWeight: 600,
  fontSize: '14px',
};

const saveBtnStyle: React.CSSProperties = {
  padding: '4px 12px',
  background: '#6366f1',
  color: '#fff',
  border: 'none',
  borderRadius: '6px',
  cursor: 'pointer',
  fontSize: '13px',
};

const cancelBtnStyle: React.CSSProperties = {
  padding: '8px 16px',
  background: '#f3f4f6',
  border: '1px solid #d1d5db',
  borderRadius: '6px',
  cursor: 'pointer',
  fontSize: '13px',
};

const editBtnStyle: React.CSSProperties = {
  padding: '4px 12px',
  background: '#f3f4f6',
  color: '#374151',
  border: '1px solid #d1d5db',
  borderRadius: '4px',
  cursor: 'pointer',
  fontSize: '13px',
  whiteSpace: 'nowrap',
};
