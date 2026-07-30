import { useState, useEffect } from 'react';

interface SoulInfo {
  soul_id: string;
  display_name: string;
  format_version: string;
  schema_version: string;
  created_at: string;
  head_event_hash: string | null;
  entity_count: number;
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

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  }
}

function isTauri(): boolean {
  return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const internals = window.__TAURI_INTERNALS__;
    if (!internals) throw new Error('Tauri not available');
    return internals.invoke(cmd, args) as Promise<T>;
  }
  throw new Error('Not running in Tauri');
}

export function App() {
  const [soul, setSoul] = useState<SoulInfo | null>(null);
  const [entities, setEntities] = useState<EntityInfo[]>([]);
  const [displayName, setDisplayName] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tauriAvailable, setTauriAvailable] = useState(false);

  useEffect(() => {
    const avail = isTauri();
    setTauriAvailable(avail);
    if (avail) {
      invoke<SoulInfo>('init_app')
        .then((s) => {
          setSoul(s);
          return invoke<EntityInfo[]>('list_entities_cmd', { soulId: s.soul_id });
        })
        .then((ents) => setEntities(ents))
        .catch(() => setSoul(null))
        .finally(() => setLoading(false));
    } else {
      setLoading(false);
    }
  }, []);

  const handleCreate = async () => {
    if (!displayName.trim()) return;
    setError(null);
    try {
      const s = await invoke<SoulInfo>('create_soul_cmd', {
        displayName: displayName.trim(),
        deviceId: 'device_001',
      });
      setSoul(s);
      setEntities([]);
    } catch (e) {
      setError(String(e));
    }
  };

  if (!tauriAvailable) {
    return (
      <div>
        <h1>SOUL</h1>
        <p>Personal Intelligence Runtime</p>
        <p>Run inside Tauri desktop app.</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div>
        <h1>SOUL</h1>
        <p>Loading...</p>
      </div>
    );
  }

  if (!soul) {
    return (
      <div>
        <h1>SOUL</h1>
        <p>Personal Intelligence Runtime</p>
        <p>No SOUL found. Create one to get started.</p>
        <div>
          <input
            type="text"
            placeholder="Your display name"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
          />
          <button onClick={handleCreate}>Create SOUL</button>
        </div>
        {error && <p style={{ color: 'red' }}>{error}</p>}
      </div>
    );
  }

  return (
    <div>
      <h1>SOUL</h1>
      <p>Personal Intelligence Runtime</p>
      <div>
        <h2>{soul.display_name}</h2>
        <p>ID: {soul.soul_id}</p>
        <p>Entities: {soul.entity_count}</p>
        <p>Created: {soul.created_at}</p>
      </div>
      <div>
        <h3>Entities ({entities.length})</h3>
        {entities.length === 0 && <p>No entities yet. Start calibration.</p>}
        {entities.map((e) => (
          <div key={e.id}>
            <strong>{e.entity_type}</strong> — {e.status}
          </div>
        ))}
      </div>
    </div>
  );
}
