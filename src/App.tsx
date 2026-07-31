import { useState, useEffect } from 'react';
import { Nav, type Tab } from './components/Nav';
import { Home } from './pages/Home';
import { Calibration } from './pages/Calibration';
import { Inbox } from './pages/Inbox';
import { Tests } from './pages/Tests';
import { ContextPage } from './pages/Context';
import { Settings } from './pages/Settings';
import { CALIBRATION_STEPS, type CalibrationAnswer } from './data/calibration';

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
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) throw new Error('Not running in Tauri');
  return internals.invoke(cmd, args) as Promise<T>;
}

function getDeviceId(): string {
  const stored = localStorage.getItem('soul_device_id');
  if (stored) return stored;
  const id = `device_${Math.random().toString(36).slice(2, 10)}`;
  localStorage.setItem('soul_device_id', id);
  return id;
}

export function App() {
  const [tab, setTab] = useState<Tab>('home');
  const [soul, setSoul] = useState<SoulInfo | null>(null);
  const [entities, setEntities] = useState<EntityInfo[]>([]);
  const [displayName, setDisplayName] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tauriAvailable, setTauriAvailable] = useState(false);
  const [showCalibration, setShowCalibration] = useState(false);
  const [calAnswers, setCalAnswers] = useState<CalibrationAnswer[]>([]);

  const deviceId = getDeviceId();

  const loadData = async () => {
    try {
      const s = await invoke<SoulInfo>('init_app');
      setSoul(s);
      const ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId: s.soul_id });
      setEntities(ents);
    } catch {
      setSoul(null);
      setEntities([]);
    }
  };

  useEffect(() => {
    const avail = isTauri();
    setTauriAvailable(avail);
    if (avail) {
      loadData().finally(() => setLoading(false));
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
        deviceId,
      });
      setSoul(s);
      setEntities([]);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleStartCalibration = async () => {
    if (!soul) return;
    setShowCalibration(true);
  };

  const handleSaveCalibration = async (step: number, answers: CalibrationAnswer[]) => {
    if (!soul) return;
    setCalAnswers(answers);
    await invoke('save_calibration_cmd', {
      soulId: soul.soul_id,
      step,
      answers: JSON.stringify(answers),
    });
  };

  const handleCalibrationComplete = async () => {
    if (!soul) return;
    setShowCalibration(false);

    for (const answer of calAnswers) {
      const question = CALIBRATION_STEPS.flatMap((s) => s.questions)
        .find((q) => q.id === answer.questionId);
      if (!question) continue;

      let payload: Record<string, unknown>;
      const val = answer.value;
      if (typeof val === 'string') {
        payload = { claim: val, source: 'calibration' };
      } else if (Array.isArray(val)) {
        payload = { claim: question.prompt, value: val, source: 'calibration' };
      } else {
        continue;
      }

      await invoke('add_entity_cmd', {
        soulId: soul.soul_id,
        entityType: question.category,
        status: 'candidate',
        data: JSON.stringify(payload),
        deviceId,
      });
    }

    const s = await invoke<SoulInfo>('get_soul_cmd', { soulId: soul.soul_id });
    if (s) setSoul(s);
    const ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId: soul.soul_id });
    setEntities(ents);
    setTab('inbox');
  };

  const handleActivate = async () => {
    if (!soul) return;
    try {
      const s = await invoke<SoulInfo>('activate_soul_cmd', { soulId: soul.soul_id });
      setSoul(s);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleConfirmEntity = async (id: string) => {
    try {
      await invoke('update_entity_cmd', {
        entityId: id,
        status: 'active',
        data: '{}',
        deviceId,
      });
      const ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId: soul?.soul_id });
      setEntities(ents);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleRejectEntity = async (id: string) => {
    try {
      await invoke('update_entity_cmd', {
        entityId: id,
        status: 'rejected',
        data: '{}',
        deviceId,
      });
      const ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId: soul?.soul_id });
      setEntities(ents);
    } catch (e) {
      setError(String(e));
    }
  };

  if (!tauriAvailable) {
    return (
      <div style={{ padding: '24px', fontFamily: 'system-ui, sans-serif' }}>
        <h1>SOUL</h1>
        <p>Personal Intelligence Runtime</p>
        <p>Run inside Tauri desktop app.</p>
      </div>
    );
  }

  const candidateCount = entities.filter((e) => e.status === 'candidate').length;

  return (
    <div style={{ padding: '16px 24px', fontFamily: 'system-ui, sans-serif', maxWidth: '720px', margin: '0 auto' }}>
      <Nav active={tab} onTab={setTab} candidateCount={candidateCount} entityCount={entities.length} />

      {error && (
        <div style={{ padding: '8px 12px', background: '#fef2f2', border: '1px solid #fecaca', borderRadius: '6px', marginBottom: '12px', color: '#dc2626', fontSize: '13px' }}>
          {error}
          <button onClick={() => setError(null)} style={{ marginLeft: '8px', background: 'none', border: 'none', cursor: 'pointer', color: '#dc2626' }}>x</button>
        </div>
      )}

      {showCalibration && soul ? (
        <Calibration
          soulId={soul.soul_id}
          initialStep={soul.calibration_step}
          initialAnswers={calAnswers}
          onSave={handleSaveCalibration}
          onComplete={handleCalibrationComplete}
          onBack={() => setShowCalibration(false)}
        />
      ) : tab === 'home' ? (
        <Home
          soul={soul}
          onCreate={handleCreate}
          onStartCalibration={handleStartCalibration}
          onContinueCalibration={() => setShowCalibration(true)}
          onActivate={handleActivate}
          displayName={displayName}
          onDisplayNameChange={setDisplayName}
          error={null}
          loading={loading}
          entityCount={entities.filter((e) => e.status === 'active').length}
          candidateCount={candidateCount}
        />
      ) : tab === 'inbox' ? (
        <Inbox entities={entities} onConfirm={handleConfirmEntity} onReject={handleRejectEntity} />
      ) : tab === 'tests' ? (
        <Tests />
      ) : tab === 'context' ? (
        <ContextPage />
      ) : (
        <Settings />
      )}
    </div>
  );
}
