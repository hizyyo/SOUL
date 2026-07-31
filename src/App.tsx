import { useState, useEffect } from 'react';
import { Nav, type Tab } from './components/Nav';
import { Home } from './pages/Home';
import { Calibration } from './pages/Calibration';
import { Inbox } from './pages/Inbox';
import { Preview } from './pages/Preview';
import { Tests } from './pages/Tests';
import { ContextPage } from './pages/Context';
import { Settings } from './pages/Settings';
import { CALIBRATION_STEPS, TOTAL_STEPS, type CalibrationAnswer } from './data/calibration';
import { compileAnswers } from './data/compile';

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
  preview_confirmed: boolean;
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

interface LastReview {
  entityId: string;
  action: 'confirmed' | 'rejected';
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

function parseCalibrationAnswers(raw: string): CalibrationAnswer[] {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return parsed.filter(
        (a): a is CalibrationAnswer =>
          a !== null &&
          typeof a === 'object' &&
          typeof (a as CalibrationAnswer).questionId === 'string' &&
          (typeof (a as CalibrationAnswer).value === 'string' ||
            Array.isArray((a as CalibrationAnswer).value)),
      );
    }
  } catch {
    // fall through to empty
  }
  return [];
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
  const [busyEntityId, setBusyEntityId] = useState<string | null>(null);
  const [lastReview, setLastReview] = useState<LastReview | null>(null);

  const deviceId = getDeviceId();

  const loadData = async () => {
    try {
      const s = await invoke<SoulInfo>('init_app');
      setSoul(s);
      let ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId: s.soul_id });
      const cal = await invoke<{ step: number; answers: string }>('get_calibration_cmd', {
        soulId: s.soul_id,
      });
      const answers = parseCalibrationAnswers(cal.answers);
      setCalAnswers(answers);
      if (cal.step >= TOTAL_STEPS && ents.length === 0 && answers.length > 0 && !s.activated) {
        const created = await createEntitiesFromAnswers(s.soul_id, answers);
        if (created.length > 0) {
          ents = created;
          const refreshed = await invoke<SoulInfo>('get_soul_cmd', { soulId: s.soul_id });
          if (refreshed) setSoul(refreshed);
        }
      }
      setEntities(ents);
      return s;
    } catch {
      setSoul(null);
      setEntities([]);
      setCalAnswers([]);
      return null;
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refreshEntities = async (soulId: string) => {
    const ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId });
    setEntities(ents);
    return ents;
  };

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
    try {
      await invoke('save_calibration_cmd', {
        soulId: soul.soul_id,
        step,
        answers: JSON.stringify(answers),
      });
    } catch (e) {
      setError(String(e));
    }
  };

  const createEntitiesFromAnswers = async (
    soulId: string,
    answers: CalibrationAnswer[],
  ): Promise<EntityInfo[]> => {
    const questions = CALIBRATION_STEPS.flatMap((s) => s.questions);
    const compiled = compileAnswers(answers, questions);
    const created: EntityInfo[] = [];
    const errors: string[] = [];
    for (const item of compiled) {
      try {
        const ent = await invoke<EntityInfo>('add_entity_cmd', {
          soulId,
          entityType: item.type,
          status: 'candidate',
          data: JSON.stringify(item.data),
          deviceId,
        });
        created.push(ent);
      } catch (e) {
        errors.push(`${item.questionId}: ${String(e)}`);
      }
    }
    if (errors.length > 0) {
      setError(`Some entities were not created: ${errors.join('; ')}`);
    }
    return created;
  };

  const handleCalibrationComplete = async (answers: CalibrationAnswer[]) => {
    if (!soul) return;
    setShowCalibration(false);
    setError(null);

    await createEntitiesFromAnswers(soul.soul_id, answers);

    try {
      const s = await invoke<SoulInfo>('get_soul_cmd', { soulId: soul.soul_id });
      if (s) setSoul(s);
      await refreshEntities(soul.soul_id);
    } catch (e) {
      setError(String(e));
    }
    setTab('preview');
  };

  const handleConfirmPreview = async () => {
    if (!soul) return;
    setError(null);
    try {
      const s = await invoke<SoulInfo>('confirm_preview_cmd', {
        soulId: soul.soul_id,
        deviceId,
      });
      setSoul(s);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleResetPreview = async () => {
    if (!soul) return;
    setError(null);
    try {
      const s = await invoke<SoulInfo>('reset_preview_cmd', {
        soulId: soul.soul_id,
        deviceId,
      });
      setSoul(s);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleActivatePreview = async (entityIds: string[]) => {
    if (!soul) return;
    setError(null);
    try {
      const s = await invoke<SoulInfo>('activate_preview_cmd', {
        soulId: soul.soul_id,
        entityIds,
        deviceId,
      });
      setSoul(s);
      await refreshEntities(soul.soul_id);
      setTab('home');
    } catch (e) {
      setError(String(e));
    }
  };

  const runStatusUpdate = async (id: string, status: string): Promise<boolean> => {
    setBusyEntityId(id);
    setError(null);
    try {
      await invoke('update_entity_cmd', {
        entityId: id,
        status,
        deviceId,
      });
      if (soul) await refreshEntities(soul.soul_id);
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setBusyEntityId(null);
    }
  };

  const handleConfirmEntity = async (id: string) => {
    const ok = await runStatusUpdate(id, 'active');
    if (ok) setLastReview({ entityId: id, action: 'confirmed' });
  };

  const handleRejectEntity = async (id: string) => {
    const ok = await runStatusUpdate(id, 'rejected');
    if (ok) setLastReview({ entityId: id, action: 'rejected' });
  };

  const handleUndoEntity = async (id: string) => {
    const ok = await runStatusUpdate(id, 'candidate');
    if (ok) setLastReview(null);
  };

  const handleEditEntity = async (id: string, claim: string) => {
    if (!soul) return;
    setBusyEntityId(id);
    setError(null);
    try {
      const ents = await refreshEntities(soul.soul_id);
      const target = ents.find((e) => e.id === id);
      if (!target) return;
      let data: Record<string, unknown> = {};
      try {
        const parsed: unknown = JSON.parse(target.data);
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          data = parsed as Record<string, unknown>;
        }
      } catch {
        data = {};
      }
      data.claim = claim.trim();
      await invoke('update_entity_cmd', {
        entityId: id,
        status: 'candidate',
        data: JSON.stringify(data),
        deviceId,
      });
      await refreshEntities(soul.soul_id);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyEntityId(null);
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
    <div
      style={{
        padding: '16px 24px',
        fontFamily: 'system-ui, sans-serif',
        maxWidth: '720px',
        margin: '0 auto',
      }}
    >
      <Nav
        active={tab}
        onTab={setTab}
        candidateCount={candidateCount}
        entityCount={entities.length}
      />

      {error && (
        <div
          style={{
            padding: '8px 12px',
            background: '#fef2f2',
            border: '1px solid #fecaca',
            borderRadius: '6px',
            marginBottom: '12px',
            color: '#dc2626',
            fontSize: '13px',
          }}
        >
          {error}
          <button
            onClick={() => setError(null)}
            style={{
              marginLeft: '8px',
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              color: '#dc2626',
            }}
          >
            x
          </button>
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
      ) : tab === 'preview' && soul && !soul.activated ? (
        <Preview
          entities={entities}
          previewConfirmed={soul.preview_confirmed}
          busyId={busyEntityId}
          onEdit={handleEditEntity}
          onConfirmPreview={handleConfirmPreview}
          onResetPreview={handleResetPreview}
          onActivate={handleActivatePreview}
          onBack={() => setTab('home')}
        />
      ) : tab === 'home' ? (
        <Home
          soul={soul}
          onCreate={handleCreate}
          onStartCalibration={handleStartCalibration}
          onContinueCalibration={() => setShowCalibration(true)}
          onGoToPreview={() => setTab('preview')}
          onGoToInbox={() => setTab('inbox')}
          onGoToSettings={() => setTab('settings')}
          displayName={displayName}
          onDisplayNameChange={setDisplayName}
          error={null}
          loading={loading}
          entityCount={entities.filter((e) => e.status === 'active').length}
          candidateCount={candidateCount}
          rejectedCount={entities.filter((e) => e.status === 'rejected').length}
          previewConfirmed={soul ? soul.preview_confirmed : false}
        />
      ) : tab === 'inbox' ? (
        <Inbox
          entities={entities}
          onConfirm={handleConfirmEntity}
          onReject={handleRejectEntity}
          onEdit={handleEditEntity}
          onUndo={handleUndoEntity}
          onDismissUndo={() => setLastReview(null)}
          lastReview={lastReview}
          busyId={busyEntityId}
        />
      ) : tab === 'tests' ? (
        <Tests />
      ) : tab === 'context' ? (
        <ContextPage />
      ) : (
        <Settings
          soul={soul}
          entities={entities}
          onDataChanged={loadData}
          onGoHome={() => setTab('home')}
        />
      )}
    </div>
  );
}
