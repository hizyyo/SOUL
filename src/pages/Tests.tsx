import { useEffect, useMemo, useState, type CSSProperties } from 'react';
import {
  SCENARIO_BANK,
  scenarioById,
  scenarioDomains,
  randomScenario,
  buildBaselineProfile,
  soulPromptFor,
  baselinePromptFor,
  compileScenarioPack,
  computeEvalStats,
  displayVariants,
  revealFor,
  shareCardText,
  EVAL_RECOMMENDED_ROUNDS,
  SHARE_MIN_ROUNDS,
  B1_PROFILE_STORAGE_KEY,
  type BlindScenario,
  type EvaluationRecord,
  type RevealResult,
} from '../data/eval';
import type { ContextEntity } from '../data/context';

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

type Phase = 'pick' | 'prompts' | 'answers' | 'choice' | 'reveal';

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

const BTN_SECONDARY: CSSProperties = {
  ...BTN,
  background: '#f3f4f6',
  color: '#333',
  border: '1px solid #d1d5db',
};

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand('copy');
    document.body.removeChild(ta);
    return ok;
  }
}

function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      style={BTN_SECONDARY}
      onClick={async () => {
        const ok = await copyText(text);
        if (ok) {
          setCopied(true);
          window.setTimeout(() => setCopied(false), 1500);
        }
      }}
    >
      {copied ? 'Copied' : label}
    </button>
  );
}

interface Props {
  soul: SoulInfo | null;
  entities: ContextEntity[];
}

export function Tests({ soul, entities }: Props) {
  const [phase, setPhase] = useState<Phase>('pick');
  const [scenario, setScenario] = useState<BlindScenario | null>(null);
  const [soulAnswer, setSoulAnswer] = useState('');
  const [baselineAnswer, setBaselineAnswer] = useState('');
  const [activeRecord, setActiveRecord] = useState<EvaluationRecord | null>(null);
  const [reveal, setReveal] = useState<RevealResult | null>(null);
  const [records, setRecords] = useState<EvaluationRecord[]>([]);
  const [b1Profile, setB1Profile] = useState<string>('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const activated = soul?.activated === true;
  const activeEntities = entities.filter((e) => e.status === 'active');

  useEffect(() => {
    const seeded = buildBaselineProfile(entities, 15, 1400);
    const stored = localStorage.getItem(B1_PROFILE_STORAGE_KEY);
    if (stored === null) {
      setB1Profile(seeded);
      localStorage.setItem(B1_PROFILE_STORAGE_KEY, seeded);
    } else {
      setB1Profile(stored);
    }
  }, [entities]);

  useEffect(() => {
    if (!soul) return;
    invoke<EvaluationRecord[]>('list_evaluations_cmd', { soulId: soul.soul_id })
      .then(setRecords)
      .catch(() => setRecords([]));
  }, [soul]);

  const stats = useMemo(() => computeEvalStats(records), [records]);
  const shareCard = useMemo(
    () => shareCardText(stats, soul?.display_name ?? 'My SOUL'),
    [stats, soul],
  );
  const usedScenarioIds = useMemo(
    () => new Set(records.filter((r) => r.user_choice).map((r) => r.scenario_id)),
    [records],
  );

  const pack = useMemo(
    () => (scenario ? compileScenarioPack(entities, scenario) : null),
    [scenario, entities],
  );

  const handlePickScenario = (s: BlindScenario) => {
    setScenario(s);
    setSoulAnswer('');
    setBaselineAnswer('');
    setReveal(null);
    setActiveRecord(null);
    setError(null);
    setPhase('prompts');
  };

  const handleRandom = () => {
    const unused = SCENARIO_BANK.filter((s) => !usedScenarioIds.has(s.id));
    const source = unused.length > 0 ? unused : SCENARIO_BANK;
    handlePickScenario(source[Math.floor(Math.random() * source.length)] ?? randomScenario());
  };

  const handleSubmitAnswers = async () => {
    if (!soul || !scenario || !pack) return;
    if (soulAnswer.trim().length === 0 || baselineAnswer.trim().length === 0) {
      setError('Paste both answers before submitting the round.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const created = await invoke<EvaluationRecord>('create_evaluation_cmd', {
        soulId: soul.soul_id,
        scenarioId: scenario.id,
        scenarioText: scenario.question,
        domain: scenario.domain,
        soulAnswer,
        baselineAnswer,
        baselineProfile: b1Profile,
        contextPack: pack.serialized,
        contextEntityIds: pack.items.map((i) => i.id),
      });
      setActiveRecord(created);
      setRecords((prev) => [created, ...prev]);
      setPhase('choice');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleChoice = async (choice: 'a' | 'b' | 'neither') => {
    if (!activeRecord) return;
    setBusy(true);
    setError(null);
    try {
      const done = await invoke<EvaluationRecord>('submit_evaluation_choice_cmd', {
        evaluationId: activeRecord.id,
        choice,
      });
      setReveal(revealFor(done, choice));
      setActiveRecord(done);
      setRecords((prev) => {
        const rest = prev.filter((r) => r.id !== done.id);
        return [done, ...rest];
      });
      setPhase('reveal');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const handleDelete = async (id: string) => {
    if (!soul) return;
    setError(null);
    try {
      await invoke('delete_evaluation_cmd', {
        soulId: soul.soul_id,
        evaluationId: id,
      });
      const list = await invoke<EvaluationRecord[]>('list_evaluations_cmd', {
        soulId: soul.soul_id,
      });
      setRecords(list);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleResetProfile = () => {
    const seeded = buildBaselineProfile(entities, 15, 1400);
    setB1Profile(seeded);
    localStorage.setItem(B1_PROFILE_STORAGE_KEY, seeded);
  };

  const progress = `Rounds: ${stats.completed} / ${EVAL_RECOMMENDED_ROUNDS}`;

  const variants = activeRecord ? displayVariants(activeRecord) : null;

  return (
    <div>
      <h2 style={{ marginTop: 0 }}>Blind Tests</h2>
      <p style={{ color: '#555', fontSize: '14px', marginTop: '-8px' }}>
        Which answer is more like you? Generate two answers in your own AI client — one with SOUL
        context, one with a short profile — and pick without knowing which is which.
      </p>

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
        </div>
      )}

      <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginBottom: '12px' }}>
        <div style={CARD} data-testid="stats-card">
          <div style={LABEL}>{progress}</div>
          <div style={{ fontSize: '22px', fontWeight: '600' }}>
            {stats.winRateLabel}
            <span style={{ fontSize: '12px', color: '#888', fontWeight: '400', marginLeft: '6px' }}>
              soul win rate
            </span>
          </div>
          <div style={{ fontSize: '13px', color: '#555', marginTop: '4px' }}>
            {stats.wins} wins · {stats.losses} losses · {stats.ties} neither ·{' '}
            <span title="95% Wilson confidence interval">95% CI {stats.confidenceLabel}</span> · p{' '}
            {stats.pValueLabel}
          </div>
        </div>
        <div style={{ ...CARD, flex: 1, minWidth: '260px' }}>
          <div style={LABEL}>Baseline profile (B1)</div>
          <textarea
            value={b1Profile}
            onChange={(e) => {
              setB1Profile(e.target.value);
              localStorage.setItem(B1_PROFILE_STORAGE_KEY, e.target.value);
            }}
            rows={4}
            style={{ width: '100%', boxSizing: 'border-box', fontSize: '12px' }}
            placeholder="Short profile the baseline prompt uses."
          />
          <button
            style={{ ...BTN_SECONDARY, marginTop: '6px', fontSize: '12px', padding: '4px 10px' }}
            onClick={handleResetProfile}
          >
            Reset from SOUL
          </button>
        </div>
      </div>

      {!activated && (
        <div style={CARD}>
          <strong>Activate your SOUL first.</strong>{' '}
          <span style={{ color: '#555' }}>
            Blind tests compare SOUL context against a baseline — activate SOUL in the Preview step
            to enable rounds.
          </span>
        </div>
      )}

      {activated && phase === 'pick' && (
        <div style={CARD}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '8px' }}>
            <strong>Pick a scenario</strong>
            <button style={BTN} onClick={handleRandom}>
              Random scenario
            </button>
            {activeEntities.length < 3 && (
              <span style={{ fontSize: '12px', color: '#b45309' }}>
                Fewer than 3 active entities — rounds will be weak. Confirm candidates in Inbox.
              </span>
            )}
          </div>
          {scenarioDomains().map((domain) => (
            <div key={domain} style={{ marginBottom: '10px' }}>
              <div style={{ ...LABEL, textTransform: 'capitalize' }}>{domain}</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                {SCENARIO_BANK.filter((s) => s.domain === domain).map((s) => (
                  <button
                    key={s.id}
                    onClick={() => handlePickScenario(s)}
                    style={{
                      textAlign: 'left',
                      border: '1px solid #e5e7eb',
                      borderRadius: '6px',
                      padding: '8px 10px',
                      background: usedScenarioIds.has(s.id) ? '#f9fafb' : '#fff',
                      color: '#333',
                      cursor: 'pointer',
                      fontSize: '13px',
                    }}
                  >
                    {s.question}
                    {usedScenarioIds.has(s.id) && (
                      <span style={{ color: '#888', fontSize: '11px', marginLeft: '8px' }}>
                        (used)
                      </span>
                    )}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {activated && phase === 'prompts' && scenario && pack && (
        <div style={CARD}>
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: '8px',
            }}
          >
            <strong>Generate two answers</strong>
            <button style={BTN_SECONDARY} onClick={() => setPhase('pick')}>
              ← Change scenario
            </button>
          </div>
          <div style={{ fontSize: '13px', color: '#444', marginBottom: '12px' }}>
            <strong>Scenario:</strong> {scenario.question}
            <div style={{ marginTop: '4px', color: '#888', fontSize: '12px' }}>
              Pack for the SOUL prompt: {pack.items.length} entities · ~{pack.tokenEstimate} tokens
              {' · '}
              <span style={{ color: pack.items.length === 0 ? '#b45309' : '#888' }}>
                {pack.items.length === 0
                  ? 'no matching entities — consider a scenario from a domain you know'
                  : `${pack.items
                      .map((i) => i.entityType)
                      .filter((v, i, a) => a.indexOf(v) === i)
                      .join(', ')}`}
              </span>
            </div>
          </div>
          <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap' }}>
            <div style={{ ...CARD, flex: 1, minWidth: '280px', marginBottom: 0 }}>
              <div style={{ ...LABEL, color: '#4f46e5' }}>Prompt 1 — with SOUL context</div>
              <pre
                style={{
                  whiteSpace: 'pre-wrap',
                  background: '#f9fafb',
                  padding: '8px',
                  borderRadius: '6px',
                  fontSize: '12px',
                  maxHeight: '220px',
                  overflow: 'auto',
                }}
              >
                {soulPromptFor({ scenario, name: soul?.display_name || 'the user', pack })}
              </pre>
              <div style={{ marginTop: '6px' }}>
                <CopyButton
                  text={soulPromptFor({ scenario, name: soul?.display_name || 'the user', pack })}
                  label="Copy prompt 1"
                />
              </div>
            </div>
            <div style={{ ...CARD, flex: 1, minWidth: '280px', marginBottom: 0 }}>
              <div style={{ ...LABEL, color: '#4f46e5' }}>Prompt 2 — with baseline profile</div>
              <pre
                style={{
                  whiteSpace: 'pre-wrap',
                  background: '#f9fafb',
                  padding: '8px',
                  borderRadius: '6px',
                  fontSize: '12px',
                  maxHeight: '220px',
                  overflow: 'auto',
                }}
              >
                {baselinePromptFor({
                  scenario,
                  profile: b1Profile,
                })}
              </pre>
              <div style={{ marginTop: '6px' }}>
                <CopyButton
                  text={baselinePromptFor({
                    scenario,
                    profile: b1Profile,
                  })}
                  label="Copy prompt 2"
                />
              </div>
            </div>
          </div>
          <p style={{ fontSize: '12px', color: '#888', marginBottom: '0' }}>
            Use the same model and settings (default temperature) for both prompts in your AI
            client, and paste both answers below.
          </p>
          <button style={{ ...BTN, marginTop: '8px' }} onClick={() => setPhase('answers')}>
            I have both answers →
          </button>
        </div>
      )}

      {activated && phase === 'answers' && scenario && (
        <div style={CARD}>
          <strong>Paste both answers</strong>
          <div style={{ display: 'flex', gap: '12px', flexWrap: 'wrap', marginTop: '8px' }}>
            <div style={{ flex: 1, minWidth: '280px' }}>
              <label style={LABEL}>Answer from Prompt 1 (SOUL context)</label>
              <textarea
                value={soulAnswer}
                onChange={(e) => setSoulAnswer(e.target.value)}
                rows={6}
                style={{ width: '100%', boxSizing: 'border-box', fontSize: '13px' }}
              />
            </div>
            <div style={{ flex: 1, minWidth: '280px' }}>
              <label style={LABEL}>Answer from Prompt 2 (baseline profile)</label>
              <textarea
                value={baselineAnswer}
                onChange={(e) => setBaselineAnswer(e.target.value)}
                rows={6}
                style={{ width: '100%', boxSizing: 'border-box', fontSize: '13px' }}
              />
            </div>
          </div>
          <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
            <button style={BTN} disabled={busy} onClick={handleSubmitAnswers}>
              Submit round
            </button>
            <button style={BTN_SECONDARY} disabled={busy} onClick={() => setPhase('prompts')}>
              ← Back
            </button>
          </div>
        </div>
      )}

      {activated && phase === 'choice' && variants && activeRecord && (
        <div style={CARD}>
          <strong>Which answer is more like you?</strong>
          <p style={{ fontSize: '13px', color: '#555', margin: '4px 0 12px' }}>
            One was generated with SOUL context, the other with a short profile. You will not see
            which is which until after you choose.
          </p>
          {variants.map((v) => (
            <button
              key={v.label}
              onClick={() => handleChoice(v.label.toLowerCase() as 'a' | 'b')}
              disabled={busy}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                border: '1px solid #e5e7eb',
                borderRadius: '8px',
                padding: '12px',
                marginBottom: '8px',
                background: '#fff',
                cursor: 'pointer',
                fontSize: '14px',
              }}
            >
              <span
                style={{
                  display: 'inline-block',
                  width: '24px',
                  height: '24px',
                  borderRadius: '50%',
                  background: '#6366f1',
                  color: '#fff',
                  textAlign: 'center',
                  lineHeight: '24px',
                  fontWeight: '700',
                  marginRight: '8px',
                  fontSize: '13px',
                }}
              >
                {v.label}
              </span>
              <span style={{ whiteSpace: 'pre-wrap' }}>{v.text}</span>
            </button>
          ))}
          <button style={BTN_SECONDARY} disabled={busy} onClick={() => handleChoice('neither')}>
            Neither
          </button>
        </div>
      )}

      {activated && phase === 'reveal' && reveal && activeRecord && (
        <div style={CARD}>
          <div
            style={{
              padding: '10px 12px',
              borderRadius: '6px',
              marginBottom: '10px',
              fontSize: '15px',
              fontWeight: '600',
              background: reveal.matchedSoul ? '#ecfdf5' : '#f3f4f6',
              color: reveal.matchedSoul ? '#065f46' : '#374151',
            }}
          >
            {reveal.matchedSoul
              ? `The answer you picked was your SOUL (${reveal.soulLabel}).`
              : reveal.choiceLabel === 'Neither'
                ? 'Neither. The SOUL answer was ' + reveal.soulLabel + '.'
                : `The answer you picked was the baseline (${reveal.soulLabel} was your SOUL).`}
          </div>
          <div style={{ display: 'flex', gap: '8px' }}>
            <button style={BTN} onClick={() => setPhase('pick')}>
              Next round →
            </button>
            <button style={BTN_SECONDARY} onClick={handleRandom}>
              Random scenario
            </button>
          </div>
        </div>
      )}

      {stats.completed > 0 && (
        <div style={CARD}>
          <strong>History</strong>
          <table
            style={{
              width: '100%',
              borderCollapse: 'collapse',
              marginTop: '8px',
              fontSize: '13px',
            }}
          >
            <thead>
              <tr style={{ textAlign: 'left', color: '#666', fontSize: '12px' }}>
                <th style={{ padding: '4px 6px' }}>Scenario</th>
                <th style={{ padding: '4px 6px' }}>Choice</th>
                <th style={{ padding: '4px 6px' }}>Result</th>
                <th style={{ padding: '4px 6px' }} />
              </tr>
            </thead>
            <tbody>
              {records
                .filter(
                  (r): r is EvaluationRecord & { user_choice: 'a' | 'b' | 'neither' } =>
                    r.user_choice !== null,
                )
                .map((r) => {
                  const res = revealFor(r, r.user_choice);
                  return (
                    <tr key={r.id} style={{ borderTop: '1px solid #f3f4f6' }}>
                      <td style={{ padding: '6px' }}>
                        {scenarioById(r.scenario_id)?.question ?? r.scenario_text}
                      </td>
                      <td style={{ padding: '6px' }}>{res.choiceLabel}</td>
                      <td style={{ padding: '6px' }}>
                        <span
                          style={{
                            color: res.matchedSoul ? '#065f46' : '#9ca3af',
                            fontWeight: res.matchedSoul ? '600' : '400',
                          }}
                        >
                          {res.matchedSoul
                            ? 'SOUL'
                            : res.choiceLabel === 'Neither'
                              ? 'tie'
                              : 'baseline'}
                        </span>
                      </td>
                      <td style={{ padding: '6px', textAlign: 'right' }}>
                        <button
                          onClick={() => handleDelete(r.id)}
                          style={{ ...BTN_SECONDARY, fontSize: '12px', padding: '2px 8px' }}
                        >
                          Delete
                        </button>
                      </td>
                    </tr>
                  );
                })}
            </tbody>
          </table>
        </div>
      )}

      {stats.completed >= SHARE_MIN_ROUNDS && shareCard && (
        <div style={CARD}>
          <strong>Share card</strong>
          <p style={{ fontSize: '12px', color: '#888', margin: '4px 0 8px' }}>
            No private questions or answers inside — only aggregates.
          </p>
          <pre
            style={{
              whiteSpace: 'pre-wrap',
              background: '#f9fafb',
              padding: '8px',
              borderRadius: '6px',
              fontSize: '12px',
            }}
          >
            {shareCard}
          </pre>
          <div style={{ marginTop: '6px' }}>
            <CopyButton text={shareCard} label="Copy share card" />
          </div>
        </div>
      )}
    </div>
  );
}
