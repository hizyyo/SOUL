import { useCallback, useEffect, useRef, useState, type CSSProperties } from 'react';
import {
  GATEWAY_CONNECTOR_OPTIONS,
  GATEWAY_DEFAULT_TTL,
  GATEWAY_MAX_TTL,
  GATEWAY_EXAMPLE_ACTION,
  SIMULATION_LABEL,
  GATEWAY_STATUS_TONE,
  gatewayStatusLabel,
  channelLabel,
  validateActionJson,
  validateChannelInput,
  capabilityState,
  shortDigest,
  type GatewayCapability,
  type GatewayChannel,
  type GatewayExecuteResult,
  type GatewayProposal,
  type GatewayReceipt,
  type GatewayStatus,
} from '../data/gateway';
import { EffectBadge } from './PolicyBadges';
import { safeErrorMessage } from '../data/safeError';
import { Modal } from '../components/Modal';
import { createLatestRequestGate } from '../data/mutations';
import { useGlobalMutation } from '../data/useMutation';

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

const BTN_SMALL: CSSProperties = {
  ...BTN,
  padding: '4px 10px',
  fontSize: '12px',
  background: '#f3f4f6',
  color: '#374151',
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
  minHeight: '200px',
  resize: 'vertical',
};

function StatusBadge({ status }: { status: GatewayStatus }) {
  const tone = GATEWAY_STATUS_TONE[status];
  return (
    <span
      style={{
        padding: '2px 8px',
        borderRadius: '999px',
        fontSize: '12px',
        fontWeight: '600',
        background: tone.bg,
        color: tone.fg,
      }}
    >
      {gatewayStatusLabel(status)}
    </span>
  );
}

function SignatureBadge({ valid }: { valid: boolean }) {
  return valid ? (
    <span style={{ color: '#047857', fontSize: '12px', fontWeight: 600 }}>✓ подписано</span>
  ) : (
    <span style={{ color: '#dc2626', fontSize: '12px', fontWeight: 600 }}>
      ✕ подпись недействительна
    </span>
  );
}

function CapabilityCard({
  cap,
  onConfirm,
  busy,
}: {
  cap: GatewayCapability;
  onConfirm?: (cap: GatewayCapability) => void;
  busy?: boolean;
}) {
  const state = capabilityState(cap);
  const stateLabel =
    state === 'used'
      ? 'использована'
      : state === 'expired'
        ? 'истекла'
        : state === 'held'
          ? 'удерживается (требует подтверждения)'
          : 'активна';
  const color =
    state === 'ready'
      ? '#047857'
      : state === 'used'
        ? '#b45309'
        : state === 'held'
          ? '#c2410c'
          : '#dc2626';
  return (
    <div
      style={{
        padding: '10px 12px',
        borderRadius: '6px',
        border: '1px solid #ddd',
        background: '#fafafa',
        fontSize: '13px',
        marginTop: '8px',
      }}
    >
      <div style={{ fontWeight: 600 }}>
        Capability {cap.id} · <span style={{ color }}>{stateLabel}</span>
      </div>
      <div style={{ color: '#555', marginTop: '4px' }}>
        action: {cap.action_id} · kind: {cap.kind}
      </div>
      <div style={{ color: '#555' }}>
        nonce: {shortDigest(cap.nonce)} · hash нагрузки: {shortDigest(cap.payload_hash)}
      </div>
      <div style={{ color: '#555' }}>
        канал (привязан): {channelLabel(cap)}
        {cap.redacted ? ' · данные скрыты (redact)' : ''}
      </div>
      <div style={{ color: '#888', fontSize: '12px' }}>
        срок: {cap.expires_at.slice(0, 19).replace('T', ' ')}
        {cap.used_at ? ` · использована: ${cap.used_at.slice(0, 19).replace('T', ' ')}` : ''}
      </div>
      <div
        style={{
          marginTop: '6px',
          display: 'flex',
          gap: '8px',
          alignItems: 'center',
          flexWrap: 'wrap',
        }}
      >
        <SignatureBadge valid={cap.signature_valid} />
        {state === 'held' && onConfirm && (
          <button
            onClick={() => onConfirm(cap)}
            style={{ ...BTN_SMALL, background: '#f59e0b', color: '#fff' }}
            disabled={busy}
          >
            Подтвердить (пользователь)
          </button>
        )}
      </div>
    </div>
  );
}

export function GatewaySection() {
  const [actionJson, setActionJson] = useState(GATEWAY_EXAMPLE_ACTION);
  const [ttl, setTtl] = useState(String(GATEWAY_DEFAULT_TTL));
  const [proposal, setProposal] = useState<GatewayProposal | null>(null);
  const [proposeError, setProposeError] = useState<string | null>(null);
  const { activeKey, run: runMutation } = useGlobalMutation();
  const busy = activeKey !== null;
  const refreshGateRef = useRef(createLatestRequestGate());

  const [channel, setChannel] = useState<GatewayChannel>(
    GATEWAY_CONNECTOR_OPTIONS[0] ?? {
      connector_id: 'demo-connector',
      account_id: 'acct-1',
      environment: 'production',
    },
  );
  const [connectors, setConnectors] = useState<GatewayChannel[]>([]);
  const [execution, setExecution] = useState<GatewayExecuteResult | null>(null);
  const [executeError, setExecuteError] = useState<string | null>(null);

  const [receipts, setReceipts] = useState<GatewayReceipt[]>([]);
  const [capabilities, setCapabilities] = useState<GatewayCapability[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Реестр каналов (добавление)
  const [newConnectorId, setNewConnectorId] = useState('');
  const [newAccountId, setNewAccountId] = useState('');
  const [newEnvironment, setNewEnvironment] = useState('');
  const [registryError, setRegistryError] = useState<string | null>(null);
  const [removeTarget, setRemoveTarget] = useState<GatewayChannel | null>(null);

  const channelOptions = connectors.length > 0 ? connectors : GATEWAY_CONNECTOR_OPTIONS;

  const refresh = useCallback(async () => {
    const request = refreshGateRef.current.begin();
    try {
      const [r, c, ch] = await Promise.all([
        invoke<GatewayReceipt[]>('list_gateway_receipts_cmd'),
        invoke<GatewayCapability[]>('list_gateway_capabilities_cmd'),
        invoke<GatewayChannel[]>('list_gateway_connectors_cmd'),
      ]);
      if (!request.isCurrent()) return;
      setReceipts(r);
      setCapabilities(c);
      setConnectors(ch);
      setLoadError(null);
    } catch {
      if (request.isCurrent()) setLoadError(safeErrorMessage('загрузить данные Gateway'));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handlePropose = async () => {
    setProposeError(null);
    setProposal(null);
    setExecution(null);
    const state = validateActionJson(actionJson);
    if (!state.ok) {
      setProposeError(state.error ?? 'Invalid action.');
      return;
    }
    await runMutation('gateway:propose', async () => {
      try {
        const ttlSeconds = ttl.trim() === '' ? null : Number(ttl);
        const result = await invoke<GatewayProposal>('gateway_propose_cmd', {
          actionJson,
          ttlSeconds,
        });
        setProposal(result);
        if (result.capability) {
          setChannel({
            connector_id: result.capability.connector_id,
            account_id: result.capability.account_id,
            environment: result.capability.environment,
          });
        }
        await refresh();
      } catch {
        setProposeError(safeErrorMessage('предложить действие'));
      }
    });
  };

  const handleExecute = async () => {
    const capability = proposal?.capability;
    if (!capability) return;
    setExecuteError(null);
    setExecution(null);
    await runMutation('gateway:execute', async () => {
      try {
        const result = await invoke<GatewayExecuteResult>('gateway_execute_cmd', {
          capabilityId: capability.id,
          connectorId: channel.connector_id,
          accountId: channel.account_id,
          environment: channel.environment,
          actionJson,
        });
        setExecution(result);
        await refresh();
      } catch {
        setExecuteError(safeErrorMessage('выполнить имитацию'));
      }
    });
  };

  const handleConfirm = async (cap: GatewayCapability) => {
    await runMutation(`gateway:confirm:${cap.id}`, async () => {
      try {
        const confirmed = await invoke<GatewayCapability>('gateway_confirm_cmd', {
          capabilityId: cap.id,
        });
        setProposal((p) => (p ? { ...p, capability: confirmed } : p));
        await refresh();
      } catch {
        setProposeError(safeErrorMessage('подтвердить capability'));
      }
    });
  };

  const handleAddConnector = async () => {
    setRegistryError(null);
    const state = validateChannelInput(newConnectorId, newAccountId, newEnvironment);
    if (!state.ok) {
      setRegistryError(state.error ?? 'Invalid channel.');
      return;
    }
    await runMutation('gateway:connector:add', async () => {
      try {
        await invoke<GatewayChannel>('gateway_add_connector_cmd', {
          connectorId: newConnectorId.trim(),
          accountId: newAccountId.trim(),
          environment: newEnvironment.trim(),
        });
        setNewConnectorId('');
        setNewAccountId('');
        setNewEnvironment('');
        await refresh();
      } catch {
        setRegistryError(safeErrorMessage('добавить канал'));
      }
    });
  };

  const handleRemoveConnector = async (c: GatewayChannel) => {
    const result = await runMutation(`gateway:connector:remove:${channelLabel(c)}`, async () => {
      setRegistryError(null);
      try {
        const removed = await invoke<boolean>('gateway_remove_connector_cmd', {
          connectorId: c.connector_id,
          accountId: c.account_id,
          environment: c.environment,
        });
        if (!removed) throw new Error('Connector not found.');
        setConnectors((current) =>
          current.filter((connector) => channelLabel(connector) !== channelLabel(c)),
        );
        await refresh();
        return true;
      } catch {
        setRegistryError(safeErrorMessage('удалить канал'));
        return false;
      }
    });
    if (result.started && result.value) setRemoveTarget(null);
  };

  return (
    <div>
      <div
        style={{
          border: '1px solid #f59e0b',
          borderRadius: '8px',
          padding: '10px 14px',
          background: '#fffbeb',
          color: '#92400e',
          fontSize: '14px',
          fontWeight: '600',
          marginBottom: '12px',
        }}
      >
        {SIMULATION_LABEL}
      </div>
      <p style={{ fontSize: '12px', color: '#666', margin: '0 0 12px' }}>
        Локальная имитация внешнего действия (§4.11): поддельный коннектор, без сети, без управления
        произвольными внешними агентами. Capability привязана к каналу и подписана локальным
        устройством; квитанции подписаны. Настоящая изоляция учётных данных — P1; имитация P0 не
        является защитой production-уровня.
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
        <div style={{ ...LABEL, fontSize: '14px' }}>Предложенное действие (SoulAction)</div>
        <p style={{ fontSize: '12px', color: '#666', margin: '0 0 8px' }}>
          Агент предлагает действие; gateway нормализует его и оценивает политиками. Поле
          payloadHash агента не доверяется — hash нагрузки вычисляется заново.
        </p>
        <label htmlFor="gateway-action-json" style={LABEL}>
          Action JSON
        </label>
        <textarea
          id="gateway-action-json"
          value={actionJson}
          onChange={(e) => setActionJson(e.target.value)}
          style={TEXTAREA}
          spellCheck={false}
        />
        <div
          style={{
            marginTop: '8px',
            display: 'flex',
            gap: '8px',
            alignItems: 'center',
            flexWrap: 'wrap',
          }}
        >
          <label style={{ fontSize: '13px', display: 'flex', gap: '6px', alignItems: 'center' }}>
            Срок (сек, 1–{GATEWAY_MAX_TTL}, по умолчанию {GATEWAY_DEFAULT_TTL}):
            <input
              type="number"
              min={1}
              max={GATEWAY_MAX_TTL}
              value={ttl}
              onChange={(e) => setTtl(e.target.value)}
              style={{ ...INPUT, width: '110px' }}
            />
          </label>
          <button onClick={() => void handlePropose()} style={BTN} disabled={busy}>
            Предложить действие
          </button>
          {proposeError && (
            <span role="alert" style={{ fontSize: '13px', color: '#dc2626' }}>
              {proposeError}
            </span>
          )}
        </div>
      </div>

      {proposal && (
        <div style={CARD}>
          <div style={{ ...LABEL, fontSize: '14px' }}>Решение политики</div>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <EffectBadge effect={proposal.decision.effect} />
            {proposal.receipt.rule_id && (
              <span style={{ fontSize: '12px', color: '#555' }}>
                rule: {proposal.receipt.rule_id}
              </span>
            )}
            {proposal.receipt.message && (
              <span style={{ fontSize: '12px', color: '#555' }}>{proposal.receipt.message}</span>
            )}
          </div>
          {proposal.capability ? (
            <CapabilityCard cap={proposal.capability} onConfirm={handleConfirm} busy={busy} />
          ) : (
            <p style={{ fontSize: '13px', color: '#b45309', margin: '8px 0 0' }}>
              Действие не разрешено — capability не выдана, коннектор не вызывался.
            </p>
          )}
        </div>
      )}

      {proposal?.capability && (
        <div style={CARD}>
          <div style={{ ...LABEL, fontSize: '14px' }}>Выполнение через поддельный коннектор</div>
          <p style={{ fontSize: '12px', color: '#666', margin: '0 0 8px' }}>
            Capability привязана к каналу {channelLabel(proposal.capability)}. Канал должен быть в
            локальном реестре имитированных коннекторов. При успехе capability сгорит (однократное
            использование); изменённая нагрузка, повтор, просрочка, другой канал — отказ.
          </p>
          <label htmlFor="gateway-channel" style={LABEL}>
            Simulated connector channel
          </label>
          <select
            id="gateway-channel"
            value={channelLabel(channel)}
            onChange={(e) => {
              const next = channelOptions.find((c) => channelLabel(c) === e.target.value);
              if (next) setChannel(next);
            }}
            style={{ ...INPUT, maxWidth: '360px', marginBottom: '8px' }}
          >
            {channelOptions.map((c) => (
              <option key={channelLabel(c)} value={channelLabel(c)}>
                {channelLabel(c)}
              </option>
            ))}
          </select>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
            <button onClick={() => void handleExecute()} style={BTN} disabled={busy}>
              Выполнить (имитация)
            </button>
            {executeError && (
              <span role="alert" style={{ fontSize: '13px', color: '#dc2626' }}>
                {executeError}
              </span>
            )}
          </div>
          {execution && (
            <div
              style={{
                marginTop: '10px',
                padding: '10px 12px',
                borderRadius: '6px',
                border: '1px solid #ddd',
                background: '#fafafa',
                fontSize: '13px',
              }}
            >
              <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
                <StatusBadge status={execution.receipt.status} />
                {execution.receipt.reason && (
                  <span style={{ color: '#6b7280' }}>{execution.receipt.reason}</span>
                )}
                <SignatureBadge valid={execution.receipt.signature_valid} />
              </div>
              {execution.receipt.message && (
                <div style={{ color: '#555', marginTop: '4px' }}>{execution.receipt.message}</div>
              )}
              <div style={{ color: '#888', fontSize: '12px', marginTop: '4px' }}>
                коннектор вызван: {execution.receipt.connector_executed ? 'да' : 'нет'}
              </div>
            </div>
          )}
        </div>
      )}

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>
          Реестр имитированных коннекторов ({connectors.length})
        </div>
        <p style={{ fontSize: '12px', color: '#666', margin: '0 0 8px' }}>
          Локальный реестр каналов для имитации: добавление/удаление управляет только поддельным
          коннектором — никаких реальных агентов. Capability без канала в реестре не выдаётся.
        </p>
        {registryError && (
          <div role="alert" style={{ fontSize: '13px', color: '#dc2626', marginBottom: '8px' }}>
            {registryError}
          </div>
        )}
        <div
          style={{
            display: 'flex',
            gap: '6px',
            flexWrap: 'wrap',
            marginBottom: '8px',
            alignItems: 'center',
          }}
        >
          <label htmlFor="gateway-connector-id" className="sr-only">
            Connector ID
          </label>
          <input
            id="gateway-connector-id"
            value={newConnectorId}
            onChange={(e) => setNewConnectorId(e.target.value)}
            placeholder="connectorId"
            style={{ ...INPUT, maxWidth: '180px' }}
          />
          <label htmlFor="gateway-account-id" className="sr-only">
            Account ID
          </label>
          <input
            id="gateway-account-id"
            value={newAccountId}
            onChange={(e) => setNewAccountId(e.target.value)}
            placeholder="accountId"
            style={{ ...INPUT, maxWidth: '140px' }}
          />
          <label htmlFor="gateway-environment" className="sr-only">
            Environment
          </label>
          <input
            id="gateway-environment"
            value={newEnvironment}
            onChange={(e) => setNewEnvironment(e.target.value)}
            placeholder="environment"
            style={{ ...INPUT, maxWidth: '140px' }}
          />
          <button onClick={() => void handleAddConnector()} style={BTN} disabled={busy}>
            Добавить канал
          </button>
        </div>
        {connectors.length === 0 && <p style={{ fontSize: '13px', color: '#666' }}>Реестр пуст.</p>}
        {channelOptions.map((c) => (
          <div
            key={channelLabel(c)}
            style={{
              display: 'flex',
              gap: '8px',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '6px 0',
              borderTop: '1px solid #eee',
              fontSize: '13px',
            }}
          >
            <span style={{ minWidth: 0, overflowWrap: 'anywhere' }}>{channelLabel(c)}</span>
            <button
              onClick={() => {
                setRegistryError(null);
                setRemoveTarget(c);
              }}
              style={BTN_SMALL}
              disabled={busy}
            >
              Удалить
            </button>
          </div>
        ))}
      </div>

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>Квитанции ({receipts.length})</div>
        {receipts.length === 0 && (
          <p style={{ fontSize: '13px', color: '#666' }}>Квитанций пока нет.</p>
        )}
        {receipts.slice(0, 10).map((r) => (
          <div
            key={r.id}
            style={{
              padding: '8px 0',
              borderTop: '1px solid #eee',
              fontSize: '13px',
            }}
          >
            <div style={{ display: 'flex', gap: '8px', alignItems: 'center', flexWrap: 'wrap' }}>
              <StatusBadge status={r.status} />
              <span style={{ fontWeight: 600 }}>{r.action_id}</span>
              <span style={{ color: '#888', fontSize: '12px' }}>
                {r.created_at.slice(0, 19).replace('T', ' ')}
              </span>
              <SignatureBadge valid={r.signature_valid} />
            </div>
            <div style={{ color: '#888', fontSize: '12px', marginTop: '2px' }}>
              {r.kind}
              {r.reason ? ` · отказ: ${r.reason}` : ''}
              {r.message ? ` · ${r.message}` : ''}
            </div>
          </div>
        ))}
      </div>

      <div style={CARD}>
        <div style={{ ...LABEL, fontSize: '14px' }}>Capabilities ({capabilities.length})</div>
        {capabilities.length === 0 && (
          <p style={{ fontSize: '13px', color: '#666' }}>Capabilities пока нет.</p>
        )}
        {capabilities.slice(0, 10).map((c) => (
          <CapabilityCard key={c.id} cap={c} onConfirm={handleConfirm} busy={busy} />
        ))}
      </div>

      {removeTarget && (
        <Modal
          title="Remove simulated connector?"
          onClose={() => setRemoveTarget(null)}
          closeOnBackdrop={!busy}
          closeOnEscape={!busy}
          ariaDescribedBy={
            registryError ? 'connector-remove-error' : 'connector-remove-description'
          }
        >
          <p id="connector-remove-description" style={{ fontSize: '13px', color: '#4b5563' }}>
            Remove <code>{channelLabel(removeTarget)}</code> from the local simulated connector
            registry? You can add it again later.
          </p>
          {registryError && (
            <p
              id="connector-remove-error"
              role="alert"
              style={{ color: '#b91c1c', fontSize: '13px' }}
            >
              {registryError}
            </p>
          )}
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
            <button onClick={() => setRemoveTarget(null)} disabled={busy} style={BTN_SMALL}>
              Cancel
            </button>
            <button
              onClick={() => void handleRemoveConnector(removeTarget)}
              disabled={busy}
              style={{ ...BTN_SMALL, background: '#dc2626', color: '#fff' }}
            >
              {busy ? 'Removing...' : 'Remove connector'}
            </button>
          </div>
        </Modal>
      )}
    </div>
  );
}
