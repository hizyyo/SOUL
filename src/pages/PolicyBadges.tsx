import type { CSSProperties } from 'react';
import { effectLabel, type Effect } from '../data/policy';

const BADGE: Record<Effect, CSSProperties> = {
  allow: { background: '#ecfdf5', color: '#047857' },
  deny: { background: '#fef2f2', color: '#dc2626' },
  require_confirmation: { background: '#fffbeb', color: '#b45309' },
  redact: { background: '#eff6ff', color: '#1d4ed8' },
};

export function EffectBadge({ effect }: { effect: Effect }) {
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
