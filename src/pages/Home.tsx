import { TOTAL_STEPS } from '../data/calibration';

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
  displayName: string;
  onDisplayNameChange: (v: string) => void;
  error: string | null;
  loading: boolean;
  entityCount: number;
  candidateCount: number;
  onGoToInbox: () => void;
}

export function Home({
  soul,
  onCreate,
  onStartCalibration,
  onContinueCalibration,
  onGoToPreview,
  displayName,
  onDisplayNameChange,
  error,
  loading,
  entityCount,
  candidateCount,
  onGoToInbox,
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
          <button
            onClick={onCreate}
            style={{
              padding: '8px 20px',
              background: '#6366f1',
              color: '#fff',
              border: 'none',
              borderRadius: '6px',
              cursor: 'pointer',
            }}
          >
            Create SOUL
          </button>
        </div>
        {error && <p style={{ color: 'red', marginTop: '8px' }}>{error}</p>}
      </div>
    );
  }

  const calibrationDone = soul.calibration_step >= TOTAL_STEPS;
  const calibrationStarted = soul.calibration_step > 0;

  const dominantCta = () => {
    if (!soul.activated && !calibrationStarted) {
      return (
        <div style={{ marginTop: '16px' }}>
          <button onClick={onStartCalibration} style={ctaBtnStyle}>
            Start Calibration (5 min)
          </button>
          <p style={{ fontSize: '13px', color: '#888', marginTop: '8px' }}>
            Build a useful local SOUL — no account, no imports, works offline.
          </p>
        </div>
      );
    }
    if (!soul.activated && !calibrationDone) {
      return (
        <div style={{ marginTop: '16px' }}>
          <button onClick={onContinueCalibration} style={ctaBtnStyle}>
            Continue Calibration
          </button>
          <p style={{ fontSize: '13px', color: '#888', marginTop: '8px' }}>
            Step {Math.min(soul.calibration_step + 1, TOTAL_STEPS)} of {TOTAL_STEPS}. Your progress
            is saved.
          </p>
        </div>
      );
    }
    if (!soul.activated) {
      return (
        <div style={{ marginTop: '16px' }}>
          <button onClick={onGoToPreview} style={{ ...ctaBtnStyle, background: '#22c55e' }}>
            Review & Activate
          </button>
          <p style={{ fontSize: '13px', color: '#888', marginTop: '8px' }}>
            Calibration is complete. Review what SOUL learned, confirm the preview, then activate.
            Sensitive items always need individual confirmation.
          </p>
        </div>
      );
    }
    return (
      <div style={{ marginTop: '16px' }}>
        <button
          style={{ ...ctaBtnStyle, background: '#d1d5db', color: '#6b7280', cursor: 'default' }}
          disabled
        >
          Connect an AI client — coming soon
        </button>
        <p style={{ fontSize: '13px', color: '#888', marginTop: '8px' }}>
          Your SOUL is active. The next step is connecting it to the AI tools you already use.
        </p>
      </div>
    );
  };

  return (
    <div>
      <h2 style={{ margin: 0 }}>{soul.display_name || 'SOUL'}</h2>
      <p style={{ color: '#666' }}>Personal Intelligence Runtime</p>

      <div style={{ marginTop: '16px', display: 'flex', gap: '24px', flexWrap: 'wrap' }}>
        <Stat label="Confirmed entities" value={entityCount} />
        <Stat label="Candidates" value={candidateCount} />
        <Stat label="Connected AI clients" value={0} />
        <Stat label="Status" value={soul.activated ? 'Active' : 'Setup'} />
      </div>

      {dominantCta()}

      <div
        style={{
          marginTop: '16px',
          display: 'flex',
          gap: '12px',
          flexWrap: 'wrap',
          fontSize: '13px',
        }}
      >
        {soul.activated && (
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              onGoToInbox();
            }}
            style={{ color: '#6366f1' }}
          >
            Review candidates ({candidateCount})
          </a>
        )}
        {soul.activated && (
          <span
            style={{ color: '#cbd5e1', cursor: 'not-allowed' }}
            title="Available in a later update"
          >
            Blind test — soon
          </span>
        )}
        {soul.activated && (
          <span
            style={{ color: '#cbd5e1', cursor: 'not-allowed' }}
            title="Available in a later update"
          >
            Improve SOUL — soon
          </span>
        )}
        {!soul.activated && (
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              onGoToInbox();
            }}
            style={{ color: '#6366f1' }}
          >
            Review candidates ({candidateCount})
          </a>
        )}
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
  background: '#6366f1',
  color: '#fff',
  border: 'none',
  borderRadius: '8px',
  cursor: 'pointer',
  fontWeight: 600,
  fontSize: '15px',
};
