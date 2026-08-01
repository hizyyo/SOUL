import { useCallback, useEffect, useState, type CSSProperties } from 'react';
import {
  POLICY_PRESETS,
  presetById,
  validateRuleJson,
  effectOfRuleJson,
  effectLabel,
  EVALUATION_EXAMPLE,
  EFFECTS,
  type Decision,
  type Effect,
  type PolicyRow,
} from '../data/policy';

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

const BADGE: Record<Effect, CSSProperties> = {
  allow: { background: '#ecfdf5', color: '#047857' },
  deny: { background: '#fef2f2', color: '#dc2626' },
  require_confirmation: { background: '#fffbeb', color: '#b45309' },
  redact: { background: '#eff6ff', color: '#1d4ed8' },
};

function EffectBadge({ effect }: { effect: Effect }) {
  return (
    <span
      style={{
        padding: '2px 8px',
        borderRadius: '999px',
        fontSize: '12px',
        fontWeight: '600',
        ...BADGE[effect],
      }}
    >
      {effectLabel(effect)}
    </span>
  );
}

export function Policies() {
  const [rows, setRows] = useState<PolicyRow[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const [presetId, setPresetId] = useState(POLICY_PRESETS[0]?.id ?? '');
  const [ruleJson, setRuleJson] = useState(POLICY_PRESETS[0]?.build() ?? '');
  const [createError, setCreateError] = useState<string | null>(null);
  const [createOk, setCreateOk] = useState<string | null>(null);

  const [actionJson, setActionJson] = useState(EVALUATION_EXAMPLE);
  const [decision, setDecision] = useState<Decision | null>(null);
  const [evalError, setEvalError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await invoke<PolicyRow[]>('list_policies_cmd'));
      setLoadError(null);
    } catch (e) {
      setLoadError(String(e));
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
    try {
      await invoke<PolicyRow>('create_policy_cmd', { ruleJson });
      setCreateOk('Rule created.');
      await load();
    } catch (e) {
      setCreateError(String(e));
    }
  };

  const handleToggle = async (row: PolicyRow) => {
    setBusyId(row.id);
    setLoadError(null);
    try {
      await invoke('set_policy_enabled_cmd', {
        policyId: row.id,
        enabled: !row.enabled,
      });
      await load();
    } catch (e) {
      setLoadError(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const handleDelete = async (id: string) => {
    setBusyId(id);
    setLoadError(null);
    try {
      await invoke('delete_policy_cmd', { policyId: id });
      await load();
    } catch (e) {
      setLoadError(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const handleEvaluate = async () => {
    setEvalError(null);
    setDecision(null);
    try {
      setDecision(await invoke<Decision>('evaluate_action_cmd', { actionJson }));
    } catch (e) {
      setEvalError(String(e));
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
        <div style={{ ...CARD, borderColor: '#fecaca', background: '#fef2f2', color: '#dc2626' }}>
          {loadError}
        </div>
      )}

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>Новое правило</div>
        <div style={{ marginBottom: '8px' }}>
          <select
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
        <textarea
          value={ruleJson}
          onChange={(e) => setRuleJson(e.target.value)}
          style={TEXTAREA}
          spellCheck={false}
        />
        <div style={{ marginTop: '8px', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <button onClick={() => void handleCreate()} style={BTN}>
            Create
          </button>
          {createOk && <span style={{ fontSize: '13px', color: '#047857' }}>{createOk}</span>}
          {createError && <span style={{ fontSize: '13px', color: '#dc2626' }}>{createError}</span>}
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
                  {row.id}
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
                  disabled={busyId === row.id}
                  onChange={() => void handleToggle(row)}
                />
                active
              </label>
              <button
                onClick={() => void handleDelete(row.id)}
                style={{ ...BTN_DANGER, opacity: busyId === row.id ? 0.5 : 1 }}
                disabled={busyId === row.id}
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
        <textarea
          value={actionJson}
          onChange={(e) => setActionJson(e.target.value)}
          style={{ ...TEXTAREA, minHeight: '200px' }}
          spellCheck={false}
        />
        <div style={{ marginTop: '8px', display: 'flex', gap: '8px', alignItems: 'center' }}>
          <button onClick={() => void handleEvaluate()} style={BTN}>
            Evaluate
          </button>
          {evalError && <span style={{ fontSize: '13px', color: '#dc2626' }}>{evalError}</span>}
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
    </div>
  );
}
