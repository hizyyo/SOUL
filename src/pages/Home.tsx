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
  onActivate: () => void;
  displayName: string;
  onDisplayNameChange: (v: string) => void;
  error: string | null;
  loading: boolean;
  entityCount: number;
  candidateCount: number;
}

export function Home({
  soul, onCreate, onStartCalibration, onContinueCalibration, onActivate,
  displayName, onDisplayNameChange, error, loading,
  entityCount, candidateCount,
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
        <div style={{ marginTop: '12px', display: 'flex', gap: '8px' }}>
          <input
            type="text"
            placeholder="Your display name"
            value={displayName}
            onChange={(e) => onDisplayNameChange(e.target.value)}
            style={{ padding: '8px 12px', border: '1px solid #ccc', borderRadius: '6px', flex: 1 }}
          />
          <button
            onClick={onCreate}
            style={{ padding: '8px 20px', background: '#6366f1', color: '#fff', border: 'none', borderRadius: '6px', cursor: 'pointer' }}
          >
            Create SOUL
          </button>
        </div>
        {error && <p style={{ color: 'red', marginTop: '8px' }}>{error}</p>}
      </div>
    );
  }

  const cta = () => {
    if (!soul.activated && soul.calibration_step === 0) {
      return (
        <div style={{ marginTop: '16px' }}>
          <button onClick={onStartCalibration} style={ctaBtnStyle}>
            Start Calibration (5 min)
          </button>
        </div>
      );
    }
    if (!soul.activated && soul.calibration_step > 0) {
      return (
        <div style={{ marginTop: '16px', display: 'flex', gap: '8px' }}>
          <button onClick={onContinueCalibration} style={ctaBtnStyle}>
            Continue Calibration
          </button>
          <button onClick={onActivate} style={{ ...ctaBtnStyle, background: '#22c55e' }}>
            Activate SOUL
          </button>
        </div>
      );
    }
    return (
      <div style={{ marginTop: '16px', padding: '12px', background: '#f0fdf4', borderRadius: '8px', border: '1px solid #bbf7d0' }}>
        <p style={{ color: '#16a34a', fontWeight: 600 }}>SOUL Active</p>
        <p style={{ fontSize: '14px', color: '#666' }}>Connect an AI client to start using SOUL.</p>
      </div>
    );
  };

  return (
    <div>
      <h2 style={{ margin: 0 }}>{soul.display_name || 'SOUL'}</h2>
      <p style={{ color: '#666' }}>Personal Intelligence Runtime</p>

      <div style={{ marginTop: '16px', display: 'flex', gap: '24px', flexWrap: 'wrap' }}>
        <Stat label="Entities" value={entityCount} />
        <Stat label="Candidates" value={candidateCount} />
        <Stat label="Status" value={soul.activated ? 'Active' : 'Setup'} />
      </div>

      {cta()}

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
