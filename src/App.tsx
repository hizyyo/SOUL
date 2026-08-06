import { useState, useEffect, useRef } from 'react';
import { Nav, type Tab } from './components/Nav';
import { Home } from './pages/Home';
import { Calibration } from './pages/Calibration';
import { Inbox } from './pages/Inbox';
import { Preview } from './pages/Preview';
import { Tests } from './pages/Tests';
import { ContextPage } from './pages/Context';
import { Policies } from './pages/Policies';
import { Settings } from './pages/Settings';
import { Demo } from './pages/Demo';
import { CALIBRATION_STEPS, TOTAL_STEPS, type CalibrationAnswer } from './data/calibration';
import { compileAnswers } from './data/compile';
import { safeErrorMessage } from './data/safeError';
import { B1_PROFILE_STORAGE_KEY } from './data/eval';
import { resolveDeviceId } from './data/identity';
import { createLatestRequestGate } from './data/mutations';
import { useGlobalMutation } from './data/useMutation';

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
  const demoOnly = new URLSearchParams(window.location.search).get('demo') === '1';
  const [tab, setTab] = useState<Tab>('home');
  const [soul, setSoul] = useState<SoulInfo | null>(null);
  const [entities, setEntities] = useState<EntityInfo[]>([]);
  const [displayName, setDisplayName] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [tauriAvailable, setTauriAvailable] = useState(false);
  const [showCalibration, setShowCalibration] = useState(false);
  const [calAnswers, setCalAnswers] = useState<CalibrationAnswer[]>([]);
  const [busyEntityIds, setBusyEntityIds] = useState<ReadonlySet<string>>(new Set());
  const [lastReview, setLastReview] = useState<LastReview | null>(null);
  const [connectedClients, setConnectedClients] = useState(0);
  const [startupFailed, setStartupFailed] = useState(false);
  const calibrationCompleteRef = useRef(false);
  const panelFocusReadyRef = useRef(false);
  const deviceIdRef = useRef<string | null>(demoOnly ? 'demo-device' : null);
  const entityRefreshGateRef = useRef(createLatestRequestGate());
  const { activeKey: activeMutation, run: runMutation } = useGlobalMutation();
  const effectiveTab = tab === 'preview' && (!soul || soul.activated) ? 'home' : tab;

  const deviceId = (): string => deviceIdRef.current ?? soul?.device_id ?? '';

  const loadConnectedClients = async () => {
    try {
      const statuses = await invoke<{ connected: boolean }[]>('detect_clients_cmd');
      setConnectedClients(statuses.filter((s) => s.connected).length);
    } catch {
      setConnectedClients(0);
    }
  };

  const loadData = async () => {
    try {
      const s = await invoke<SoulInfo | null>('init_app');
      if (!s) {
        setSoul(null);
        setEntities([]);
        setCalAnswers([]);
        setStartupFailed(false);
        localStorage.removeItem(B1_PROFILE_STORAGE_KEY);
        return null;
      }
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
      setStartupFailed(false);
      return s;
    } catch {
      setSoul(null);
      setEntities([]);
      setCalAnswers([]);
      setStartupFailed(true);
      setError(safeErrorMessage('загрузить локальные данные'));
      return null;
    }
  };

  useEffect(() => {
    if (demoOnly) {
      setLoading(false);
      return;
    }
    const avail = isTauri();
    setTauriAvailable(avail);
    if (avail) {
      try {
        deviceIdRef.current = resolveDeviceId(localStorage);
      } catch {
        setStartupFailed(true);
        setError(safeErrorMessage('получить идентификатор установки'));
        setLoading(false);
        return;
      }
      void loadData()
        .then(() => loadConnectedClients())
        .finally(() => setLoading(false));
    } else {
      setLoading(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (demoOnly) return;
    void loadConnectedClients();
  }, [demoOnly, effectiveTab]);

  useEffect(() => {
    if (!panelFocusReadyRef.current) {
      panelFocusReadyRef.current = true;
      return;
    }
    document.getElementById('main-panel')?.focus();
  }, [effectiveTab, showCalibration]);

  const refreshEntities = async (soulId: string) => {
    const request = entityRefreshGateRef.current.begin();
    const ents = await invoke<EntityInfo[]>('list_entities_cmd', { soulId });
    if (request.isCurrent()) setEntities(ents);
    return ents;
  };

  const setEntityPending = (id: string, pending: boolean) => {
    setBusyEntityIds((current) => {
      const next = new Set(current);
      if (pending) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const runEntityMutation = async (
    id: string,
    action: () => Promise<boolean>,
  ): Promise<boolean> => {
    const result = await runMutation(`entity:${id}`, async () => {
      setEntityPending(id, true);
      try {
        return await action();
      } finally {
        setEntityPending(id, false);
      }
    });
    return result.started ? result.value : false;
  };

  const handleCreate = async () => {
    if (!displayName.trim()) return;
    await runMutation('soul:create', async () => {
      setError(null);
      try {
        const s = await invoke<SoulInfo>('create_soul_cmd', {
          displayName: displayName.trim(),
          deviceId: deviceId(),
        });
        setSoul(s);
        setEntities([]);
      } catch {
        setError(safeErrorMessage('создать SOUL'));
      }
    });
  };

  const handleStartCalibration = async () => {
    if (!soul) return;
    setShowCalibration(true);
  };

  const handleSaveCalibration = async (step: number, answers: CalibrationAnswer[]) => {
    if (!soul) return;
    const result = await runMutation('calibration:save', async () => {
      try {
        await invoke('save_calibration_cmd', {
          soulId: soul.soul_id,
          step,
          answers: JSON.stringify(answers),
        });
        setCalAnswers(answers);
      } catch (e) {
        setError(safeErrorMessage('сохранить калибровку'));
        throw e;
      }
    });
    if (!result.started) throw new Error('Another mutation is already running.');
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
          deviceId: deviceId(),
        });
        created.push(ent);
      } catch {
        errors.push(item.questionId);
      }
    }
    if (errors.length > 0) {
      setError(
        `Some calibration items could not be created (${errors.length}). Your answers remain saved; retry from Home.`,
      );
    }
    return created;
  };

  const handleCalibrationComplete = async (answers: CalibrationAnswer[]) => {
    if (!soul || calibrationCompleteRef.current) return;
    calibrationCompleteRef.current = true;
    try {
      const result = await runMutation('calibration:complete', async () => {
        setError(null);
        try {
          await createEntitiesFromAnswers(soul.soul_id, answers);
          const [s] = await Promise.all([
            invoke<SoulInfo>('get_soul_cmd', { soulId: soul.soul_id }),
            refreshEntities(soul.soul_id),
          ]);
          if (s) setSoul(s);
          setShowCalibration(false);
          setTab('preview');
        } catch (e) {
          setError(safeErrorMessage('завершить калибровку'));
          throw e;
        }
      });
      if (!result.started) throw new Error('Another mutation is already running.');
    } finally {
      calibrationCompleteRef.current = false;
    }
  };

  const handleConfirmPreview = async () => {
    if (!soul) return;
    await runMutation('preview:confirm', async () => {
      setError(null);
      try {
        const s = await invoke<SoulInfo>('confirm_preview_cmd', {
          soulId: soul.soul_id,
          deviceId: deviceId(),
        });
        setSoul(s);
      } catch {
        setError(safeErrorMessage('подтвердить preview'));
      }
    });
  };

  const handleResetPreview = async () => {
    if (!soul) return;
    await runMutation('preview:reset', async () => {
      setError(null);
      try {
        const s = await invoke<SoulInfo>('reset_preview_cmd', {
          soulId: soul.soul_id,
          deviceId: deviceId(),
        });
        setSoul(s);
      } catch {
        setError(safeErrorMessage('сбросить preview'));
      }
    });
  };

  const handleActivatePreview = async (entityIds: string[]) => {
    if (!soul) return;
    await runMutation('preview:activate', async () => {
      setError(null);
      try {
        const s = await invoke<SoulInfo>('activate_preview_cmd', {
          soulId: soul.soul_id,
          entityIds,
          deviceId: deviceId(),
        });
        await refreshEntities(soul.soul_id);
        setSoul(s);
        setTab('home');
      } catch {
        setError(safeErrorMessage('активировать SOUL'));
      }
    });
  };

  const runStatusUpdate = async (id: string, status: string): Promise<boolean> => {
    return runEntityMutation(id, async () => {
      setError(null);
      try {
        const updated = await invoke<EntityInfo>('update_entity_cmd', {
          soulId: soul?.soul_id ?? '',
          entityId: id,
          status,
          deviceId: deviceId(),
        });
        setEntities((current) => current.map((entity) => (entity.id === id ? updated : entity)));
        if (soul) {
          try {
            await refreshEntities(soul.soul_id);
          } catch {
            setError('The entity was updated, but the full entity list could not be refreshed.');
          }
        }
        return true;
      } catch {
        setError(safeErrorMessage('обновить запись'));
        return false;
      }
    });
  };

  const handleConfirmEntity = async (id: string) => {
    const ok = await runStatusUpdate(id, 'active');
    if (ok) setLastReview({ entityId: id, action: 'confirmed' });
    return ok;
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
    await runEntityMutation(id, async () => {
      setError(null);
      try {
        const target = entities.find((e) => e.id === id);
        if (!target) return false;
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
        const updated = await invoke<EntityInfo>('update_entity_cmd', {
          soulId: soul.soul_id,
          entityId: id,
          status: 'candidate',
          data: JSON.stringify(data),
          deviceId: deviceId(),
        });
        setEntities((current) => current.map((entity) => (entity.id === id ? updated : entity)));
        try {
          await refreshEntities(soul.soul_id);
        } catch {
          setError('The entity was updated, but the full entity list could not be refreshed.');
        }
        return true;
      } catch {
        setError(safeErrorMessage('изменить запись'));
        return false;
      }
    });
  };

  if (demoOnly) {
    return (
      <div className="app-shell">
        <Demo onExit={() => window.location.assign(window.location.pathname)} />
      </div>
    );
  }

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
    <div className="app-shell">
      <Nav
        active={effectiveTab}
        onTab={setTab}
        onDemo={() => window.location.assign(`${window.location.pathname}?demo=1`)}
        candidateCount={candidateCount}
        entityCount={entities.length}
        showPreview={soul !== null && !soul.activated}
      />

      {error && (
        <div className="app-error" role="alert">
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

      <main
        id="main-panel"
        role="tabpanel"
        tabIndex={-1}
        aria-labelledby={`nav-tab-${effectiveTab}`}
      >
        {startupFailed ? (
          <section aria-labelledby="startup-error-title">
            <h2 id="startup-error-title">Не удалось открыть локальные данные</h2>
            <p>
              SOUL ничего не изменял. Проверьте, что приложение имеет доступ к своим локальным
              файлам, затем повторите.
            </p>
            <button
              onClick={() => {
                setLoading(true);
                void loadData()
                  .then(() => loadConnectedClients())
                  .finally(() => setLoading(false));
              }}
            >
              Повторить загрузку
            </button>
          </section>
        ) : showCalibration && soul ? (
          <Calibration
            soulId={soul.soul_id}
            initialStep={soul.calibration_step}
            initialAnswers={calAnswers}
            onSave={handleSaveCalibration}
            onComplete={handleCalibrationComplete}
            onBack={() => setShowCalibration(false)}
            globallyBusy={activeMutation !== null}
          />
        ) : effectiveTab === 'preview' && soul && !soul.activated ? (
          <Preview
            entities={entities}
            previewConfirmed={soul.preview_confirmed}
            busyIds={busyEntityIds}
            globallyBusy={activeMutation !== null}
            onEdit={handleEditEntity}
            onConfirmPreview={handleConfirmPreview}
            onResetPreview={handleResetPreview}
            onActivate={handleActivatePreview}
            onBack={() => setTab('home')}
          />
        ) : effectiveTab === 'home' ? (
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
            connectedClients={connectedClients}
            busy={activeMutation !== null}
          />
        ) : effectiveTab === 'inbox' ? (
          <Inbox
            entities={entities}
            onConfirm={handleConfirmEntity}
            onReject={handleRejectEntity}
            onEdit={handleEditEntity}
            onUndo={handleUndoEntity}
            onDismissUndo={() => setLastReview(null)}
            lastReview={lastReview}
            busyIds={busyEntityIds}
            globallyBusy={activeMutation !== null}
          />
        ) : effectiveTab === 'tests' ? (
          <Tests soul={soul} entities={entities} />
        ) : effectiveTab === 'context' ? (
          <ContextPage soul={soul} entities={entities} />
        ) : effectiveTab === 'policies' ? (
          <Policies />
        ) : (
          <Settings
            soul={soul}
            entities={entities}
            onDataChanged={loadData}
            onGoHome={() => setTab('home')}
          />
        )}
      </main>
    </div>
  );
}
