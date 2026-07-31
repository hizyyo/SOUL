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

interface InboxProps {
  entities: ReviewEntity[];
  onConfirm: (id: string) => void;
  onReject: (id: string) => void;
  onEdit: (id: string, claim: string) => void;
  onUndo: (id: string) => void;
  onDismissUndo: () => void;
  lastReview: { entityId: string; action: 'confirmed' | 'rejected' } | null;
  busyId: string | null;
}

interface ConfirmTarget {
  entity: ReviewEntity;
}

export function Inbox({
  entities,
  onConfirm,
  onReject,
  onEdit,
  onUndo,
  onDismissUndo,
  lastReview,
  busyId,
}: InboxProps) {
  const candidates = rankCandidates(entities.filter((e) => e.status === 'candidate'));
  const active = entities
    .filter((e) => e.status === 'active')
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
  const rejected = entities
    .filter((e) => e.status === 'rejected')
    .sort((a, b) => b.created_at.localeCompare(a.created_at));

  const [confirmTarget, setConfirmTarget] = useState<ConfirmTarget | null>(null);
  const [boundaryAck, setBoundaryAck] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [draft, setDraft] = useState('');

  const handleConfirmClick = (e: ReviewEntity) => {
    if (requiresExplicitConfirm(e)) {
      setBoundaryAck(false);
      setConfirmTarget({ entity: e });
      return;
    }
    onConfirm(e.id);
  };

  const handleEditStart = (e: ReviewEntity) => {
    setEditingId(e.id);
    setDraft(claimOf(e));
  };

  const handleEditSave = (id: string) => {
    const target = entities.find((e) => e.id === id);
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
      <h2 style={{ margin: '0 0 4px' }}>Inbox</h2>
      <p style={{ color: '#888', fontSize: '14px', margin: '0 0 16px' }}>
        Review what SOUL learned about you. Everything here stays on your device.
      </p>

      {lastReview && (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '8px 12px',
            background: '#eff6ff',
            border: '1px solid #bfdbfe',
            borderRadius: '8px',
            marginBottom: '16px',
            fontSize: '13px',
          }}
        >
          <span style={{ color: '#1d4ed8' }}>
            {lastReview.action === 'confirmed' ? 'Entity confirmed.' : 'Entity rejected.'}
          </span>
          <button
            onClick={() => onUndo(lastReview.entityId)}
            disabled={busyId === lastReview.entityId}
            style={{
              marginLeft: 'auto',
              background: '#fff',
              border: '1px solid #93c5fd',
              borderRadius: '6px',
              padding: '4px 12px',
              cursor: busyId === lastReview.entityId ? 'default' : 'pointer',
              color: '#1d4ed8',
              fontWeight: 600,
            }}
          >
            Undo
          </button>
          <button
            onClick={onDismissUndo}
            style={{ background: 'none', border: 'none', cursor: 'pointer', color: '#6b7280' }}
            aria-label="Dismiss"
          >
            x
          </button>
        </div>
      )}

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
          No candidates to review.
        </div>
      ) : (
        <div
          style={{ display: 'flex', flexDirection: 'column', gap: '10px', marginBottom: '16px' }}
        >
          <p style={{ color: '#888', fontSize: '13px', margin: 0 }}>
            {candidates.length} candidate{candidates.length !== 1 ? 's' : ''} — sorted by importance
            and risk
          </p>
          {candidates.map((e) => (
            <CandidateCard
              key={e.id}
              entity={e}
              busy={busyId === e.id}
              editing={editingId === e.id}
              draft={draft}
              onDraftChange={setDraft}
              onConfirm={() => handleConfirmClick(e)}
              onReject={() => onReject(e.id)}
              onEditStart={() => handleEditStart(e)}
              onEditSave={() => handleEditSave(e.id)}
              onEditCancel={() => setEditingId(null)}
            />
          ))}
        </div>
      )}

      {rejected.length > 0 && (
        <div style={{ marginBottom: '16px' }}>
          <h3 style={{ fontSize: '14px', margin: '0 0 8px', color: '#6b7280' }}>
            Rejected ({rejected.length})
          </h3>
          {rejected.map((e) => (
            <div
              key={e.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                padding: '8px 12px',
                border: '1px solid #e5e7eb',
                borderRadius: '8px',
                background: '#fafafa',
                marginBottom: '6px',
              }}
            >
              <span
                style={{
                  fontSize: '11px',
                  color: '#9ca3af',
                  fontWeight: 600,
                  textTransform: 'uppercase',
                  minWidth: '72px',
                }}
              >
                {e.entity_type}
              </span>
              <span style={{ fontSize: '13px', flex: 1, color: '#4b5563' }}>
                {maskText(claimOf(e))}
              </span>
              <button
                onClick={() => onUndo(e.id)}
                disabled={busyId === e.id}
                style={{
                  background: '#fff',
                  border: '1px solid #d1d5db',
                  borderRadius: '6px',
                  padding: '4px 10px',
                  cursor: busyId === e.id ? 'default' : 'pointer',
                  fontSize: '12px',
                }}
              >
                Restore
              </button>
            </div>
          ))}
        </div>
      )}

      {active.length > 0 && (
        <div>
          <h3 style={{ fontSize: '14px', margin: '0 0 8px', color: '#6b7280' }}>
            Active ({active.length})
          </h3>
          {active.map((e) => (
            <div
              key={e.id}
              style={{
                padding: '8px 12px',
                marginBottom: '6px',
                border: '1px solid #e5e7eb',
                borderRadius: '8px',
                background: '#fafafa',
              }}
            >
              <span
                style={{
                  fontSize: '11px',
                  color: '#6366f1',
                  fontWeight: 600,
                  textTransform: 'uppercase',
                }}
              >
                {e.entity_type}
              </span>
              <p style={{ margin: '4px 0 0', fontSize: '14px' }}>{maskText(claimOf(e))}</p>
            </div>
          ))}
        </div>
      )}

      {confirmTarget && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.4)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 50,
            padding: '16px',
          }}
          onClick={() => setConfirmTarget(null)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label="Explicit confirmation"
            onClick={(ev) => ev.stopPropagation()}
            style={{
              background: '#fff',
              borderRadius: '10px',
              padding: '20px',
              maxWidth: '440px',
              width: '100%',
            }}
          >
            <span
              style={{
                fontSize: '11px',
                padding: '2px 8px',
                borderRadius: '4px',
                background: '#fef3c7',
                color: '#b45309',
                fontWeight: 700,
                textTransform: 'uppercase',
              }}
            >
              {confirmTarget.entity.entity_type === 'boundary' ? 'Boundary' : 'Sensitive'}
            </span>
            <p style={{ margin: '10px 0', fontWeight: 600, fontSize: '15px' }}>
              {maskText(claimOf(confirmTarget.entity))}
            </p>
            <p style={{ fontSize: '13px', color: '#6b7280', margin: '0 0 12px' }}>
              This is a boundary or sensitive item. It limits what AI can do on your behalf and
              needs explicit confirmation.
            </p>
            <label
              style={{
                display: 'flex',
                gap: '8px',
                alignItems: 'start',
                fontSize: '13px',
                cursor: 'pointer',
                marginBottom: '16px',
              }}
            >
              <input
                type="checkbox"
                checked={boundaryAck}
                onChange={(ev) => setBoundaryAck(ev.target.checked)}
                style={{ marginTop: '2px' }}
              />
              <span>I understand this will apply to AI actions.</span>
            </label>
            <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button
                onClick={() => setConfirmTarget(null)}
                style={{
                  padding: '6px 16px',
                  background: '#f3f4f6',
                  border: '1px solid #d1d5db',
                  borderRadius: '6px',
                  cursor: 'pointer',
                }}
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  onConfirm(confirmTarget.entity.id);
                  setConfirmTarget(null);
                }}
                disabled={!boundaryAck || busyId === confirmTarget.entity.id}
                style={{
                  padding: '6px 16px',
                  background: '#22c55e',
                  color: '#fff',
                  border: 'none',
                  borderRadius: '6px',
                  cursor: busyId === confirmTarget.entity.id ? 'default' : 'pointer',
                  fontWeight: 600,
                  opacity: boundaryAck ? 1 : 0.5,
                }}
              >
                Confirm
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function CandidateCard({
  entity,
  busy,
  editing,
  draft,
  onDraftChange,
  onConfirm,
  onReject,
  onEditStart,
  onEditSave,
  onEditCancel,
}: {
  entity: ReviewEntity;
  busy: boolean;
  editing: boolean;
  draft: string;
  onDraftChange: (v: string) => void;
  onConfirm: () => void;
  onReject: () => void;
  onEditStart: () => void;
  onEditSave: () => void;
  onEditCancel: () => void;
}) {
  const data = parseEntityData(entity.data);
  const confidence = data.confidence;
  const domains = data.scope?.domains ?? [];
  const isBoundary = entity.entity_type === 'boundary' || data.risk === true;

  return (
    <div
      style={{
        padding: '12px',
        border: isBoundary ? '1px solid #fcd34d' : '1px solid #e5e7eb',
        borderRadius: '8px',
        background: '#fff',
      }}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'start',
          gap: '8px',
          flexWrap: 'wrap',
        }}
      >
        <div style={{ minWidth: '0', flex: '1 1 260px' }}>
          <span
            style={{
              fontSize: '11px',
              padding: '2px 8px',
              borderRadius: '4px',
              background: isBoundary ? '#fef3c7' : '#eef2ff',
              color: isBoundary ? '#b45309' : '#6366f1',
              fontWeight: 600,
              textTransform: 'uppercase',
            }}
          >
            {entity.entity_type}
          </span>
          {editing ? (
            <div style={{ marginTop: '8px' }}>
              <textarea
                value={draft}
                onChange={(ev) => onDraftChange(ev.target.value)}
                style={{
                  width: '100%',
                  minHeight: '56px',
                  padding: '8px',
                  border: '1px solid #d1d5db',
                  borderRadius: '6px',
                  resize: 'vertical',
                  fontSize: '14px',
                }}
                aria-label="Edit statement"
              />
              <div style={{ display: 'flex', gap: '6px', marginTop: '6px' }}>
                <button
                  onClick={onEditSave}
                  style={{
                    padding: '4px 12px',
                    background: '#6366f1',
                    color: '#fff',
                    border: 'none',
                    borderRadius: '6px',
                    cursor: 'pointer',
                    fontSize: '13px',
                  }}
                >
                  Save
                </button>
                <button
                  onClick={onEditCancel}
                  style={{
                    padding: '4px 12px',
                    background: '#f3f4f6',
                    border: '1px solid #d1d5db',
                    borderRadius: '6px',
                    cursor: 'pointer',
                    fontSize: '13px',
                  }}
                >
                  Cancel
                </button>
              </div>
            </div>
          ) : (
            <p
              style={{
                margin: '8px 0 4px',
                fontWeight: 500,
                fontSize: '14px',
                overflowWrap: 'anywhere',
              }}
            >
              {maskText(claimOf(entity))}
            </p>
          )}
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
              Confidence: {confidence !== undefined ? `${Math.round(confidence * 100)}%` : '—'}
            </span>
            <span>Source: {formatSourceDate(entity.created_at)}</span>
            {domains.length > 0 && <span>Scope: {domains.join(', ')}</span>}
          </div>
          {data.evidence && !editing && (
            <p
              style={{
                margin: '6px 0 0',
                fontSize: '12px',
                color: '#9ca3af',
                fontStyle: 'italic',
                overflowWrap: 'anywhere',
              }}
            >
              Evidence: {maskText(data.evidence)}
            </p>
          )}
        </div>
        <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
          <button onClick={onConfirm} disabled={busy} style={confirmBtnStyle}>
            Confirm
          </button>
          <button onClick={onEditStart} disabled={busy} style={editBtnStyle}>
            Edit
          </button>
          <button onClick={onReject} disabled={busy} style={rejectBtnStyle}>
            Reject
          </button>
        </div>
      </div>
    </div>
  );
}

const confirmBtnStyle: React.CSSProperties = {
  padding: '4px 12px',
  background: '#22c55e',
  color: '#fff',
  border: 'none',
  borderRadius: '4px',
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
};

const rejectBtnStyle: React.CSSProperties = {
  padding: '4px 12px',
  background: '#ef4444',
  color: '#fff',
  border: 'none',
  borderRadius: '4px',
  cursor: 'pointer',
  fontSize: '13px',
};
