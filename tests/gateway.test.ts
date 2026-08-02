import { describe, it, expect } from 'vitest';
import {
  GATEWAY_STATUSES,
  GATEWAY_CONNECTOR_OPTIONS,
  GATEWAY_EXAMPLE_ACTION,
  GATEWAY_DEFAULT_TTL,
  GATEWAY_MAX_TTL,
  SIMULATION_LABEL,
  gatewayStatusLabel,
  channelLabel,
  validateActionJson,
  capabilityState,
  shortDigest,
  type GatewayCapability,
} from '../src/data/gateway';
import { EFFECTS } from '../src/data/policy';

describe('simulation label (§4.11)', () => {
  it('is the exact required string', () => {
    expect(SIMULATION_LABEL).toBe('Имитация: внешнее действие не выполнялось.');
  });
});

describe('status labels', () => {
  it('labels every status and returns a tone', () => {
    for (const status of GATEWAY_STATUSES) {
      expect(gatewayStatusLabel(status).length).toBeGreaterThan(0);
    }
    expect(GATEWAY_STATUSES).toHaveLength(6);
  });
});

describe('connector registry mirror', () => {
  it('covers the seed combos from gateway.rs', () => {
    const keys = GATEWAY_CONNECTOR_OPTIONS.map(channelLabel);
    expect(keys).toContain('demo-connector · acct-1 · production');
    expect(keys).toContain('demo-connector · acct-1 · staging');
    expect(keys).toContain('demo-connector · acct-2 · production');
    expect(keys).toContain('sandbox-connector · acct-1 · development');
  });

  it('example action channel exists in the registry', () => {
    const action = JSON.parse(GATEWAY_EXAMPLE_ACTION) as Record<string, string>;
    const channel = {
      connectorId: action.connectorId,
      accountId: action.accountId,
      environment: action.environment,
    };
    expect(GATEWAY_CONNECTOR_OPTIONS).toContainEqual(channel);
  });
});

describe('example action', () => {
  it('is valid and matches the §4.11 demo (purchase $600, production)', () => {
    expect(validateActionJson(GATEWAY_EXAMPLE_ACTION).ok).toBe(true);
    const action = JSON.parse(GATEWAY_EXAMPLE_ACTION) as Record<string, unknown>;
    expect(action.kind).toBe('purchase.create');
    expect(action.amount).toBe(600);
    expect(action.environment).toBe('production');
  });
});

describe('validateActionJson', () => {
  const good = (): string =>
    JSON.stringify({
      actionId: 'a1',
      kind: 'notes.create',
      actor: 'agent-1',
      connectorId: 'demo-connector',
      accountId: 'acct-1',
      environment: 'production',
    });

  it('accepts a valid action', () => {
    const state = validateActionJson(good());
    expect(state.ok).toBe(true);
    expect(state.error).toBeNull();
  });

  it('rejects invalid JSON and non-objects', () => {
    expect(validateActionJson('{nope').ok).toBe(false);
    expect(validateActionJson('[]').ok).toBe(false);
    expect(validateActionJson('42').ok).toBe(false);
  });

  it('rejects empty required fields', () => {
    const blank = good().replace('"kind":"notes.create"', '"kind":"   "');
    expect(validateActionJson(blank).ok).toBe(false);
    expect(validateActionJson(good().replace('"accountId":"acct-1"', '')).ok).toBe(false);
  });
});

describe('capabilityState', () => {
  const base: GatewayCapability = {
    id: 'cap_1',
    action_id: 'a1',
    kind: 'notes.create',
    payload_hash: 'h',
    nonce: 'n',
    expires_at: new Date(Date.now() + 60_000).toISOString(),
    created_at: new Date().toISOString(),
    used_at: null,
  };

  it('distinguishes ready, used and expired', () => {
    expect(capabilityState(base)).toBe('ready');
    expect(capabilityState({ ...base, used_at: new Date().toISOString() })).toBe('used');
    expect(
      capabilityState({ ...base, expires_at: new Date(Date.now() - 60_000).toISOString() }),
    ).toBe('expired');
  });
});

describe('helpers', () => {
  it('shortens long digests and keeps short ones', () => {
    expect(shortDigest('abc').endsWith('…')).toBe(false);
    const long = 'a'.repeat(64);
    expect(shortDigest(long)).toMatch(/^a{12}…$/);
  });

  it('mirrors TTL constants from gateway.rs', () => {
    expect(GATEWAY_DEFAULT_TTL).toBe(300);
    expect(GATEWAY_MAX_TTL).toBe(3_600);
  });

  it('receipt decision effects are known policy effects', () => {
    // Тип GatewayReceipt.decision_effect = Effect — все значения в EFFECTS.
    expect(EFFECTS).toContain('allow');
    expect(EFFECTS).toContain('deny');
  });
});
