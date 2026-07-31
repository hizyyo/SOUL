import { useEffect, useMemo, useRef, useState } from 'react';
import {
  compileContext,
  defaultQuery,
  collectDomains,
  CONTEXT_STANDARD_TOKENS,
  CONTEXT_HARD_MAX_TOKENS,
  type ContextQuery,
  type ContextEntity,
  type SensitivityLevel,
} from '../data/context';

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

const SENSITIVITY_OPTIONS = ['public', 'internal', 'private', 'sensitive', 'restricted'] as const;
const STATUS_OPTIONS = [
  'active',
  'disputed',
  'superseded',
  'expired',
  'candidate',
  'rejected',
] as const;

function parseClaim(data: string): string {
  try {
    const parsed: unknown = JSON.parse(data);
    if (
      parsed &&
      typeof parsed === 'object' &&
      typeof (parsed as { claim?: unknown }).claim === 'string'
    ) {
      return (parsed as { claim: string }).claim;
    }
  } catch {
    // fall through
  }
  return data;
}

function Chip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: '3px 10px',
        borderRadius: '12px',
        border: active ? '1px solid #2563eb' : '1px solid #d1d5db',
        background: active ? '#eff6ff' : '#fff',
        color: active ? '#1d4ed8' : '#374151',
        fontSize: '12px',
        cursor: 'pointer',
        marginRight: '6px',
        marginBottom: '6px',
      }}
    >
      {children}
    </button>
  );
}

export function ContextPage({ soul, entities }: { soul: SoulInfo | null; entities: EntityInfo[] }) {
  const [search, setSearch] = useState('');
  const [hits, setHits] = useState<EntityInfo[]>([]);
  const [searching, setSearching] = useState(false);
  const [domains, setDomains] = useState<string[]>([]);
  const [sensitivity, setSensitivity] = useState<SensitivityLevel[]>([]);
  const [statuses, setStatuses] = useState<string[]>([]);
  const [maxTokens, setMaxTokens] = useState(CONTEXT_STANDARD_TOKENS);
  const timerRef = useRef<number | null>(null);

  const allDomains = useMemo(() => collectDomains(entities), [entities]);

  useEffect(() => {
    if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    const text = search.trim();
    if (!soul || !text) {
      setHits([]);
      setSearching(false);
      return;
    }
    setSearching(true);
    timerRef.current = window.setTimeout(() => {
      invoke<EntityInfo[]>('search_entities_cmd', {
        soulId: soul.soul_id,
        query: text,
        limit: 20,
      })
        .then(setHits)
        .catch(() => setHits([]))
        .finally(() => setSearching(false));
    }, 250);
    return () => {
      if (timerRef.current !== null) window.clearTimeout(timerRef.current);
    };
  }, [search, soul]);

  const query: ContextQuery = useMemo(() => {
    const q = defaultQuery();
    q.text = search.trim();
    q.domains = domains;
    q.sensitivity =
      sensitivity.length === SENSITIVITY_OPTIONS.length
        ? [...SENSITIVITY_OPTIONS]
        : sensitivity.length > 0
          ? [...sensitivity]
          : [];
    q.statuses = statuses;
    q.maxTokens = maxTokens;
    return q;
  }, [search, domains, sensitivity, statuses, maxTokens]);

  const pack = useMemo(() => compileContext(entities as ContextEntity[], query), [entities, query]);

  const toggle = (list: string[], set: (v: string[]) => void, value: string) => {
    if (list.includes(value)) set(list.filter((v) => v !== value));
    else set([...list, value]);
  };

  const toggleSensitivity = (value: SensitivityLevel) => {
    if (sensitivity.includes(value)) setSensitivity(sensitivity.filter((v) => v !== value));
    else setSensitivity([...sensitivity, value]);
  };

  const count = entities.length;

  return (
    <div>
      <h2>Context</h2>
      <p style={{ color: '#888', fontSize: '13px' }}>
        Minimal allowed task context: deterministic, token-packed, never exceeds the budget.
      </p>

      {count === 0 && (
        <p style={{ color: '#888', fontSize: '13px' }}>
          No entities yet — finish calibration first.
        </p>
      )}

      <div style={{ display: 'flex', gap: '8px', marginBottom: '8px' }}>
        <input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Full-text search (FTS5): try 'concise'"
          style={{
            flex: 1,
            padding: '8px 10px',
            border: '1px solid #d1d5db',
            borderRadius: '6px',
            fontSize: '13px',
          }}
        />
        <span
          style={{
            color: searching ? '#2563eb' : '#9ca3af',
            fontSize: '12px',
            alignSelf: 'center',
          }}
        >
          {searching ? '…' : hits.length > 0 ? `${hits.length}` : ''}
        </span>
      </div>

      {hits.length > 0 && (
        <div
          style={{
            border: '1px solid #e5e7eb',
            borderRadius: '6px',
            padding: '8px 12px',
            marginBottom: '12px',
            background: '#fafafa',
          }}
        >
          <div style={{ fontSize: '11px', color: '#9ca3af', marginBottom: '4px' }}>FTS results</div>
          {hits.map((hit) => (
            <div key={hit.id} style={{ fontSize: '13px', padding: '2px 0' }}>
              <span style={{ color: '#2563eb', fontFamily: 'monospace', fontSize: '11px' }}>
                {hit.entity_type}/{hit.status}
              </span>{' '}
              {parseClaim(hit.data)}
            </div>
          ))}
        </div>
      )}

      <div style={{ marginBottom: '8px' }}>
        <div style={{ fontSize: '11px', color: '#9ca3af', marginBottom: '4px' }}>Domains</div>
        {allDomains.length === 0 && (
          <span style={{ fontSize: '12px', color: '#9ca3af' }}>none</span>
        )}
        {allDomains.map((d) => (
          <Chip key={d} active={domains.includes(d)} onClick={() => toggle(domains, setDomains, d)}>
            {d}
          </Chip>
        ))}
      </div>

      <div style={{ marginBottom: '8px' }}>
        <div style={{ fontSize: '11px', color: '#9ca3af', marginBottom: '4px' }}>Sensitivity</div>
        {SENSITIVITY_OPTIONS.map((s) => (
          <Chip key={s} active={sensitivity.includes(s)} onClick={() => toggleSensitivity(s)}>
            {s}
          </Chip>
        ))}
      </div>

      <div style={{ marginBottom: '8px' }}>
        <div style={{ fontSize: '11px', color: '#9ca3af', marginBottom: '4px' }}>Status</div>
        {STATUS_OPTIONS.map((s) => (
          <Chip
            key={s}
            active={statuses.includes(s)}
            onClick={() => toggle(statuses, setStatuses, s)}
          >
            {s}
          </Chip>
        ))}
      </div>

      <div style={{ marginBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
        <label style={{ fontSize: '12px', color: '#374151' }}>Token budget</label>
        <input
          type="range"
          min={100}
          max={CONTEXT_HARD_MAX_TOKENS}
          step={100}
          value={maxTokens}
          onChange={(e) => setMaxTokens(Number(e.target.value))}
          style={{ flex: 1 }}
        />
        <span style={{ fontSize: '12px', fontFamily: 'monospace', color: '#374151' }}>
          {maxTokens}
        </span>
      </div>

      <div
        style={{
          border: '1px solid #e5e7eb',
          borderRadius: '6px',
          overflow: 'hidden',
          marginBottom: '12px',
        }}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            padding: '6px 12px',
            background: '#fafafa',
            borderBottom: '1px solid #e5e7eb',
          }}
        >
          <span style={{ fontSize: '12px', color: '#374151', fontWeight: 600 }}>Context pack</span>
          <span
            style={{
              fontSize: '12px',
              fontFamily: 'monospace',
              color: pack.tokenEstimate > pack.maxTokens ? '#dc2626' : '#16a34a',
            }}
          >
            {pack.tokenEstimate} / {pack.maxTokens} tokens · {pack.items.length} entities
          </span>
        </div>
        <pre
          style={{
            margin: 0,
            padding: '12px',
            fontSize: '12px',
            lineHeight: '1.5',
            fontFamily: 'ui-monospace, monospace',
            whiteSpace: 'pre-wrap',
            wordBreak: 'break-word',
            color: '#111827',
            maxHeight: '420px',
            overflowY: 'auto',
          }}
        >
          {pack.serialized}
        </pre>
      </div>
    </div>
  );
}
