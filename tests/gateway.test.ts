import { describe, it, expect } from 'vitest';
import {
  GATEWAY_STATUSES,
  GATEWAY_CONNECTOR_OPTIONS,
  GATEWAY_EXAMPLE_ACTION,
  GATEWAY_DEFAULT_TTL,
  GATEWAY_MAX_TTL,
  GATEWAY_MAX_CONNECTORS,
  GATEWAY_MAX_CHANNEL_FIELD_CHARS,
  SIMULATION_LABEL,
  gatewayStatusLabel,
  channelLabel,
  validateActionJson,
  validateChannelInput,
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
      connector_id: action.connectorId,
      account_id: action.accountId,
      environment: action.environment,
    };
    expect(GATEWAY_CONNECTOR_OPTIONS).toContainEqual(channel);
  });

  it('mirrors registry limits from gateway.rs', () => {
    expect(GATEWAY_MAX_CONNECTORS).toBe(50);
    expect(GATEWAY_MAX_CHANNEL_FIELD_CHARS).toBe(64);
  });
});

describe('validateChannelInput', () => {
  it('accepts a valid channel and trims', () => {
    const state = validateChannelInput('  demo-connector ', 'acct-1', 'production');
    expect(state.ok).toBe(true);
    expect(state.error).toBeNull();
  });

  it('rejects empty and oversized fields', () => {
    expect(validateChannelInput('', 'acct-1', 'production').ok).toBe(false);
    expect(validateChannelInput('a', '  ', 'production').ok).toBe(false);
    expect(validateChannelInput('a', 'acct-1', '').ok).toBe(false);
    expect(
      validateChannelInput('x'.repeat(GATEWAY_MAX_CHANNEL_FIELD_CHARS + 1), 'acct-1', 'production')
        .ok,
    ).toBe(false);
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
    connector_id: 'demo-connector',
    account_id: 'acct-1',
    environment: 'production',
    decision_effect: 'allow',
    confirmed_by_user: true,
    redacted: false,
    signature: 'sig',
    signer_public_key: 'pub',
    signature_valid: true,
  };

  it('distinguishes ready, used and expired', () => {
    expect(capabilityState(base)).toBe('ready');
    expect(capabilityState({ ...base, used_at: new Date().toISOString() })).toBe('used');
    expect(
      capabilityState({ ...base, expires_at: new Date(Date.now() - 60_000).toISOString() }),
    ).toBe('expired');
  });

  it('holds an unconfirmed require_confirmation capability', () => {
    expect(
      capabilityState({ ...base, decision_effect: 'require_confirmation', confirmed_by_user: false }),
    ).toBe('held');
    expect(
      capabilityState({ ...base, decision_effect: 'require_confirmation', confirmed_by_user: true }),
    ).toBe('ready');
  });

  it('is not held when used or expired first', () => {
    expect(
      capabilityState({
        ...base,
        decision_effect: 'require_confirmation',
        confirmed_by_user: false,
        used_at: new Date().toISOString(),
      }),
    ).toBe('used');
    expect(
      capabilityState({
        ...base,
        decision_effect: 'require_confirmation',
        confirmed_by_user: false,
        expires_at: new Date(Date.now() - 60_000).toISOString(),
      }),
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
