interface EntityInfo {
  id: string;
  entity_type: string;
  status: string;
  data: string;
  created_at: string;
}

interface InboxProps {
  entities: EntityInfo[];
  onConfirm: (id: string) => void;
  onReject: (id: string) => void;
}

export function Inbox({ entities, onConfirm, onReject }: InboxProps) {
  const candidates = entities.filter((e) => e.status === 'candidate');
  const active = entities.filter((e) => e.status === 'active');

  if (candidates.length === 0) {
    return (
      <div>
        <h2>Inbox</h2>
        <p style={{ color: '#888' }}>No candidates to review.</p>
        {active.length > 0 && (
          <div style={{ marginTop: '16px' }}>
            <h3>Active entities ({active.length})</h3>
            {active.map((e) => (
              <EntityCard key={e.id} entity={e} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div>
      <h2>Inbox</h2>
      <p style={{ color: '#888', fontSize: '14px' }}>{candidates.length} candidate{candidates.length !== 1 ? 's' : ''} to review</p>
      {candidates.map((e) => (
        <div key={e.id} style={{
          padding: '12px', marginBottom: '8px', border: '1px solid #e5e7eb',
          borderRadius: '8px', background: '#fff',
        }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'start' }}>
            <div>
              <span style={{
                fontSize: '11px', padding: '2px 8px', borderRadius: '4px',
                background: '#eef2ff', color: '#6366f1', fontWeight: 600,
                textTransform: 'uppercase',
              }}>
                {e.entity_type}
              </span>
              <p style={{ margin: '8px 0 4px', fontWeight: 500 }}>{parseClaim(e.data)}</p>
              <p style={{ fontSize: '12px', color: '#888', margin: 0 }}>
                Confidence: {parseConfidence(e.data)}
              </p>
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              <button onClick={() => onConfirm(e.id)} style={confirmBtnStyle}>Confirm</button>
              <button onClick={() => onReject(e.id)} style={rejectBtnStyle}>Reject</button>
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}

function EntityCard({ entity }: { entity: EntityInfo }) {
  return (
    <div style={{ padding: '8px 12px', marginBottom: '4px', border: '1px solid #e5e7eb', borderRadius: '6px', background: '#fafafa' }}>
      <span style={{ fontSize: '11px', color: '#6366f1', fontWeight: 600, textTransform: 'uppercase' }}>{entity.entity_type}</span>
      <p style={{ margin: '4px 0 0', fontSize: '14px' }}>{parseClaim(entity.data)}</p>
    </div>
  );
}

function parseClaim(data: string): string {
  try {
    const parsed = JSON.parse(data);
    return parsed.claim ?? parsed.value ?? data;
  } catch {
    return data;
  }
}

function parseConfidence(data: string): string {
  try {
    const parsed = JSON.parse(data);
    return parsed.confidence ? `${Math.round(parsed.confidence * 100)}%` : 'N/A';
  } catch {
    return 'N/A';
  }
}

const confirmBtnStyle: React.CSSProperties = {
  padding: '4px 12px', background: '#22c55e', color: '#fff',
  border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '13px',
};

const rejectBtnStyle: React.CSSProperties = {
  padding: '4px 12px', background: '#ef4444', color: '#fff',
  border: 'none', borderRadius: '4px', cursor: 'pointer', fontSize: '13px',
};
