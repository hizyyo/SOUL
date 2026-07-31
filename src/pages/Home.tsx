import { TOTAL_STEPS } from '../data/calibration';
import { buildControlCenter, type ActionItem } from '../data/control';

interface SoulInfo {
  soul_id: string;
  display_name: string;
  activated: boolean;
  calibration_step: number;
  entity_count: number;
  created_at: string;
}

interface HomeProps {
  soul: SoulInfo | null;
  onCreate: () => void;
  onStartCalibration: () => void;
  onContinueCalibration: () => void;
  onGoToPreview: () => void;
  onGoToInbox: () => void;
  onGoToSettings: () => void;
  displayName: string;
  onDisplayNameChange: (v: string) => void;
  error: string | null;
  loading: boolean;
  entityCount: number;
  candidateCount: number;
  rejectedCount: number;
  previewConfirmed: boolean;
}

export function Home({
  soul,
  onCreate,
  onStartCalibration,
  onContinueCalibration,
  onGoToPreview,
  onGoToInbox,
  onGoToSettings,
  displayName,
  onDisplayNameChange,
  error,
  loading,
  entityCount,
  candidateCount,
  rejectedCount,
  previewConfirmed,
}: HomeProps) {
  if (loading) {
    return (
      <div>
        <h2>SOUL</h2>
        <p>Loading...</p>
      </div>
    );
  }

  if (!soul) {
    return (
      <div>
        <h2>SOUL</h2>
        <p style={{ color: '#666' }}>Personal Intelligence Runtime</p>
        <p>No SOUL found. Create one to get started.</p>
        <div style={{ marginTop: '12px', display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
          <input
            type="text"
            placeholder="Your display name"
            value={displayName}
            onChange={(e) => onDisplayNameChange(e.target.value)}
            style={{
              padding: '8px 12px',
              border: '1px solid #ccc',
              borderRadius: '6px',
              flex: '1 1 180px',
            }}
          />
          <button onClick={onCreate} style={ctaBtnStyle}>
            Create SOUL
          </button>
        </div>
        {error && <p style={{ color: 'red', marginTop: '8px' }}>{error}</p>}
      </div>
    );
  }

  const center = buildControlCenter({
    hasSoul: true,
    activated: soul.activated,
    calibrationStep: soul.calibration_step,
    totalSteps: TOTAL_STEPS,
    previewConfirmed,
    candidateCount,
  });

  const runAction = (a: ActionItem) => {
    if (a.disabled) return;
    if (a.target === 'calibration') {
      if (center.state === 'start-calibration') onStartCalibration();
      else onContinueCalibration();
      return;
    }
    if (a.target === 'preview') onGoToPreview();
    if (a.target === 'inbox') onGoToInbox();
    if (a.target === 'settings') onGoToSettings();
  };

  const primary = center.next;

  return (
    <div>
      <h2 style={{ margin: 0 }}>{soul.display_name || 'SOUL'}</h2>
      <p style={{ color: '#666' }}>Personal Intelligence Runtime</p>

      <div style={{ marginTop: '16px', display: 'flex', gap: '24px', flexWrap: 'wrap' }}>
        <Stat label="Status" value={center.statusLabel} />
        <Stat label="Confirmed entities" value={entityCount} />
        <Stat label="Candidates" value={candidateCount} />
        <Stat label="Rejected" value={rejectedCount} />
        <Stat label="Connected AI clients" value={0} />
        <Stat
          label="Calibration"
          value={
            soul.calibration_step >= TOTAL_STEPS
              ? 'Done'
              : `${soul.calibration_step}/${TOTAL_STEPS}`
          }
        />
      </div>

      <div
        style={{
          marginTop: '20px',
          padding: '18px',
          border: primary.disabled ? '1px solid #e5e7eb' : '1px solid #c7d2fe',
          borderRadius: '10px',
          background: primary.disabled ? '#fafafa' : '#f5f6ff',
        }}
      >
        <div style={{ fontSize: '12px', color: '#6b7280', fontWeight: 600, marginBottom: '8px' }}>
          NEXT STEP
        </div>
        <button
          onClick={() => runAction(primary)}
          disabled={primary.disabled}
          style={{
            ...ctaBtnStyle,
            background: primary.disabled ? '#d1d5db' : soul.activated ? '#22c55e' : '#6366f1',
            color: primary.disabled ? '#6b7280' : '#fff',
            cursor: primary.disabled ? 'default' : 'pointer',
          }}
        >
          {primary.label}
        </button>
        <p style={{ fontSize: '13px', color: '#888', marginTop: '10px' }}>{primary.note}</p>
      </div>

      <div style={{ marginTop: '16px' }}>
        <div style={{ fontSize: '12px', color: '#6b7280', fontWeight: 600, marginBottom: '8px' }}>
          OTHER ACTIONS
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {center.secondary.map((a) => (
            <button
              key={a.id}
              onClick={() => runAction(a)}
              disabled={a.disabled}
              title={a.note}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '10px',
                padding: '10px 12px',
                border: '1px solid #e5e7eb',
                borderRadius: '8px',
                background: a.disabled ? '#fafafa' : '#fff',
                cursor: a.disabled ? 'default' : 'pointer',
                textAlign: 'left',
                width: '100%',
              }}
            >
              <span
                style={{
                  fontSize: '13px',
                  fontWeight: 600,
                  color: a.disabled ? '#9ca3af' : '#374151',
                  flex: '1',
                }}
              >
                {a.label}
              </span>
              <span style={{ fontSize: '12px', color: a.disabled ? '#d1d5db' : '#9ca3af' }}>
                {a.disabled ? 'soon' : '→'}
              </span>
            </button>
          ))}
        </div>
      </div>

      <details style={{ marginTop: '20px', opacity: 0.6 }}>
        <summary style={{ cursor: 'pointer', fontSize: '13px' }}>SOUL info</summary>
        <pre style={{ fontSize: '11px', marginTop: '8px' }}>{JSON.stringify(soul, null, 2)}</pre>
      </details>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <div style={{ textAlign: 'center' }}>
      <div style={{ fontSize: '24px', fontWeight: 700, color: '#6366f1' }}>{value}</div>
      <div style={{ fontSize: '12px', color: '#888' }}>{label}</div>
    </div>
  );
}

const ctaBtnStyle: React.CSSProperties = {
  padding: '10px 24px',
  border: 'none',
  borderRadius: '8px',
  fontWeight: 600,
  fontSize: '15px',
};
