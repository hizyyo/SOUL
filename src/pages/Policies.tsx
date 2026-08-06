import { useCallback, useEffect, useRef, useState, type CSSProperties } from 'react';
import {
  POLICY_PRESETS,
  presetById,
  validateRuleJson,
  effectOfRuleJson,
  EVALUATION_EXAMPLE,
  EFFECTS,
  type Decision,
  type PolicyRow,
} from '../data/policy';
import { EffectBadge } from './PolicyBadges';
import { GatewaySection } from './GatewaySection';
import { safeErrorMessage } from '../data/safeError';
import { Modal } from '../components/Modal';
import { useGlobalMutation } from '../data/useMutation';
import { createLatestRequestGate } from '../data/mutations';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
    };
  }
}

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const internals = window.__TAURI_INTERNALS__;
  if (!internals) throw new Error('Not running in Tauri');
  return internals.invoke(cmd, args) as Promise<T>;
}

const CARD: CSSProperties = {
  border: '1px solid #ddd',
  borderRadius: '8px',
  padding: '12px 16px',
  marginBottom: '12px',
  background: '#fff',
};

const LABEL: CSSProperties = {
  display: 'block',
  fontSize: '12px',
  color: '#666',
  marginBottom: '4px',
  fontWeight: '600',
};

const BTN: CSSProperties = {
  padding: '8px 16px',
  border: 'none',
  borderRadius: '6px',
  background: '#6366f1',
  color: '#fff',
  cursor: 'pointer',
  fontSize: '14px',
};

const BTN_DANGER: CSSProperties = {
  padding: '6px 12px',
  border: 'none',
  borderRadius: '6px',
  background: '#ef4444',
  color: '#fff',
  cursor: 'pointer',
  fontSize: '13px',
};

const INPUT: CSSProperties = {
  width: '100%',
  padding: '8px',
  border: '1px solid #ccc',
  borderRadius: '6px',
  fontSize: '13px',
  boxSizing: 'border-box',
};

const TEXTAREA: CSSProperties = {
  ...INPUT,
  fontFamily: 'ui-monospace, monospace',
  minHeight: '140px',
  resize: 'vertical',
};

export function Policies() {
  const [rows, setRows] = useState<PolicyRow[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PolicyRow | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const { activeKey, run: runMutation } = useGlobalMutation();
  const globallyBusy = activeKey !== null;
  const loadGateRef = useRef(createLatestRequestGate());

  const [presetId, setPresetId] = useState(POLICY_PRESETS[0]?.id ?? '');
  const [ruleJson, setRuleJson] = useState(POLICY_PRESETS[0]?.build() ?? '');
  const [createError, setCreateError] = useState<string | null>(null);
  const [createOk, setCreateOk] = useState<string | null>(null);

  const [actionJson, setActionJson] = useState(EVALUATION_EXAMPLE);
  const [decision, setDecision] = useState<Decision | null>(null);
  const [evalError, setEvalError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const request = loadGateRef.current.begin();
    try {
      const nextRows = await invoke<PolicyRow[]>('list_policies_cmd');
      if (!request.isCurrent()) return;
      setRows(nextRows);
      setLoadError(null);
    } catch {
      if (request.isCurrent()) setLoadError(safeErrorMessage('загрузить политики'));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const applyPreset = (id: string) => {
    setPresetId(id);
    const preset = presetById(id);
    if (preset) setRuleJson(preset.build());
    setCreateError(null);
    setCreateOk(null);
  };

  const handleCreate = async () => {
    setCreateError(null);
    setCreateOk(null);
    const state = validateRuleJson(ruleJson);
    if (!state.ok) {
      setCreateError(state.error ?? 'Invalid rule.');
      return;
    }
    await runMutation('policy:create', async () => {
      try {
        const created = await invoke<PolicyRow>('create_policy_cmd', { ruleJson });
        setRows((current) => [created, ...current.filter((row) => row.id !== created.id)]);
        setCreateOk('Rule created.');
        await load();
      } catch {
        setCreateError(safeErrorMessage('создать правило'));
      }
    });
  };

  const handleToggle = async (row: PolicyRow) => {
    await runMutation(`policy:toggle:${row.id}`, async () => {
      setBusyId(row.id);
      setLoadError(null);
      try {
        const updated = await invoke<PolicyRow>('set_policy_enabled_cmd', {
          policyId: row.id,
          enabled: !row.enabled,
        });
        setRows((current) => current.map((item) => (item.id === updated.id ? updated : item)));
        await load();
      } catch {
        setLoadError(safeErrorMessage('обновить правило'));
      } finally {
        setBusyId(null);
      }
    });
  };

  const handleDelete = async (row: PolicyRow) => {
    const result = await runMutation(`policy:delete:${row.id}`, async () => {
      setBusyId(row.id);
      setDeleteError(null);
      try {
        await invoke('delete_policy_cmd', { policyId: row.id });
        setRows((current) => current.filter((item) => item.id !== row.id));
        await load();
        return true;
      } catch {
        setDeleteError(safeErrorMessage('удалить правило'));
        return false;
      } finally {
        setBusyId(null);
      }
    });
    if (result.started && result.value) setDeleteTarget(null);
  };

  const handleEvaluate = async () => {
    setEvalError(null);
    setDecision(null);
    try {
      setDecision(await invoke<Decision>('evaluate_action_cmd', { actionJson }));
    } catch {
      setEvalError(safeErrorMessage('оценить действие'));
    }
  };

  return (
    <div>
      <h2 style={{ marginTop: 0 }}>Policies</h2>
      <p style={{ fontSize: '13px', color: '#555', marginTop: 0 }}>
        Детерминированный DSL политик (ULTRA_MVP §4.10): только типизированные условия, эффекты{' '}
        {EFFECTS.join(', ')}. Без eval, без сети, без регулярных выражений.
      </p>

      {loadError && (
        <div
          role="alert"
          style={{ ...CARD, borderColor: '#fecaca', background: '#fef2f2', color: '#dc2626' }}
        >
          {loadError}
        </div>
      )}

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>Новое правило</div>
        <div style={{ marginBottom: '8px' }}>
          <label htmlFor="policy-preset" style={LABEL}>
            Preset
          </label>
          <select
            id="policy-preset"
            value={presetId}
            onChange={(e) => applyPreset(e.target.value)}
            style={{ ...INPUT, maxWidth: '340px' }}
          >
            {POLICY_PRESETS.map((p) => (
              <option key={p.id} value={p.id}>
                {p.label}
              </option>
            ))}
          </select>
          <p style={{ fontSize: '12px', color: '#666', margin: '6px 0 0' }}>
            {presetById(presetId)?.description}
          </p>
        </div>
        <label htmlFor="policy-rule-json" style={LABEL}>
          Rule JSON
        </label>
        <textarea
          id="policy-rule-json"
          value={ruleJson}
          onChange={(e) => setRuleJson(e.target.value)}
          style={TEXTAREA}
          spellCheck={false}
        />
        <div style={{ marginTop: '8px', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <button onClick={() => void handleCreate()} style={BTN} disabled={globallyBusy}>
            Create
          </button>
          {createOk && <span style={{ fontSize: '13px', color: '#047857' }}>{createOk}</span>}
          {createError && (
            <span role="alert" style={{ fontSize: '13px', color: '#dc2626' }}>
              {createError}
            </span>
          )}
        </div>
      </div>

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>Правила ({rows.length})</div>
        {rows.length === 0 && <p style={{ fontSize: '13px', color: '#666' }}>Правил пока нет.</p>}
        {rows.map((row) => {
          const effect = effectOfRuleJson(row.rule_json);
          return (
            <div
              key={row.id}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
                padding: '8px 0',
                borderTop: '1px solid #eee',
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontSize: '14px',
                    fontWeight: '600',
                    display: 'flex',
                    gap: '8px',
                    alignItems: 'center',
                  }}
                >
                  <span style={{ minWidth: 0, overflowWrap: 'anywhere' }}>{row.id}</span>
                  {effect && <EffectBadge effect={effect} />}
                </div>
                <div style={{ fontSize: '12px', color: '#888' }}>
                  priority {row.priority} · updated {row.updated_at.slice(0, 19).replace('T', ' ')}
                </div>
              </div>
              <label
                style={{ fontSize: '13px', display: 'flex', gap: '4px', alignItems: 'center' }}
              >
                <input
                  type="checkbox"
                  checked={row.enabled}
                  disabled={globallyBusy || busyId === row.id}
                  onChange={() => void handleToggle(row)}
                />
                active
              </label>
              <button
                onClick={() => {
                  setDeleteError(null);
                  setDeleteTarget(row);
                }}
                style={{ ...BTN_DANGER, opacity: busyId === row.id ? 0.5 : 1 }}
                disabled={globallyBusy || busyId === row.id}
              >
                Delete
              </button>
            </div>
          );
        })}
      </div>

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>Оценить действие (Gateway-демо §4.11)</div>
        <p style={{ fontSize: '12px', color: '#666', margin: '0 0 8px' }}>
          Пример: покупка на $600 в production — правило high-value должно потребовать
          подтверждение, необратимость без подтверждения — запрет.
        </p>
        <label htmlFor="policy-action-json" style={LABEL}>
          Action JSON
        </label>
        <textarea
          id="policy-action-json"
          value={actionJson}
          onChange={(e) => setActionJson(e.target.value)}
          style={{ ...TEXTAREA, minHeight: '200px' }}
          spellCheck={false}
        />
        <div style={{ marginTop: '8px', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <button onClick={() => void handleEvaluate()} style={BTN} disabled={globallyBusy}>
            Evaluate
          </button>
          {evalError && (
            <span role="alert" style={{ fontSize: '13px', color: '#dc2626' }}>
              {evalError}
            </span>
          )}
        </div>
        {decision && (
          <div
            style={{
              marginTop: '10px',
              padding: '10px 12px',
              borderRadius: '6px',
              border: '1px solid #ddd',
              background: '#fafafa',
              fontSize: '14px',
            }}
          >
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
              Решение: <EffectBadge effect={decision.effect} />
            </div>
            {decision.rule_id && (
              <div style={{ fontSize: '12px', color: '#555', marginTop: '4px' }}>
                rule: {decision.rule_id}
              </div>
            )}
            {decision.message && (
              <div style={{ fontSize: '12px', color: '#555' }}>{decision.message}</div>
            )}
          </div>
        )}
      </div>

      <p style={{ fontSize: '12px', color: '#888' }}>
        Демо-правила сеются один раз за жизнь хранилища: удалённые правила не воскресают. Оценка
        никогда не вызывает сеть и не имеет побочных эффектов.
      </p>

      <h3 style={{ marginBottom: 8 }}>Имитированный Gateway (§4.11)</h3>
      <GatewaySection />

      {deleteTarget && (
        <Modal
          title="Delete policy?"
          onClose={() => setDeleteTarget(null)}
          closeOnBackdrop={!globallyBusy}
          closeOnEscape={!globallyBusy}
          ariaDescribedBy={deleteError ? 'policy-delete-error' : 'policy-delete-description'}
          titleStyle={{ color: '#b91c1c' }}
        >
          <p id="policy-delete-description" style={{ fontSize: '13px', color: '#4b5563' }}>
            Delete <code>{deleteTarget.id}</code>? This removes the rule immediately. Recreate it
            from its preset or JSON if you need it again.
          </p>
          {deleteError && (
            <p id="policy-delete-error" role="alert" style={{ color: '#b91c1c', fontSize: '13px' }}>
              {deleteError}
            </p>
          )}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
            <button
              onClick={() => setDeleteTarget(null)}
              disabled={globallyBusy}
              style={{ ...BTN, background: '#f3f4f6', color: '#374151' }}
            >
              Cancel
            </button>
            <button
              onClick={() => void handleDelete(deleteTarget)}
              disabled={globallyBusy}
              style={BTN_DANGER}
            >
              {busyId === deleteTarget.id ? 'Deleting...' : 'Delete policy'}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
