export type Tab = 'home' | 'inbox' | 'tests' | 'context' | 'settings';

interface NavProps {
  active: Tab;
  onTab: (tab: Tab) => void;
  candidateCount: number;
  entityCount: number;
}

const TABS: { id: Tab; label: string }[] = [
  { id: 'home', label: 'Home' },
  { id: 'inbox', label: 'Inbox' },
  { id: 'tests', label: 'Tests' },
  { id: 'context', label: 'Context' },
  { id: 'settings', label: 'Settings' },
];

export function Nav({ active, onTab, candidateCount, entityCount }: NavProps) {
  return (
    <nav style={{ display: 'flex', gap: '4px', padding: '8px', borderBottom: '1px solid #ddd', marginBottom: '16px' }}>
      {TABS.map((t) => (
        <button
          key={t.id}
          onClick={() => onTab(t.id)}
          style={{
            padding: '6px 16px',
            border: 'none',
            borderRadius: '6px',
            background: active === t.id ? '#6366f1' : 'transparent',
            color: active === t.id ? '#fff' : '#333',
            cursor: 'pointer',
            fontWeight: active === t.id ? '600' : '400',
            fontSize: '14px',
            position: 'relative',
          }}
        >
          {t.label}
          {t.id === 'inbox' && candidateCount > 0 && (
            <span style={{
              position: 'absolute', top: '-4px', right: '-4px',
              background: '#ef4444', color: '#fff', borderRadius: '50%',
              width: '18px', height: '18px', fontSize: '11px',
              display: 'flex', alignItems: 'center', justifyContent: 'center',
            }}>
              {candidateCount}
            </span>
          )}
        </button>
      ))}
      <span style={{ marginLeft: 'auto', fontSize: '12px', color: '#888', alignSelf: 'center' }}>
        {entityCount} entities
      </span>
    </nav>
  );
}
