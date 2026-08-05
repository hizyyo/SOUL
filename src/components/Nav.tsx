import { useRef } from 'react';
import { tabIndexForKey } from '../data/nav';

export type Tab =
  'home' | 'inbox' | 'preview' | 'tests' | 'context' | 'policies' | 'settings' | 'demo';

interface NavProps {
  active: Tab;
  onTab: (tab: Tab) => void;
  onDemo: () => void;
  candidateCount: number;
  entityCount: number;
}

const TABS: { id: Tab; label: string }[] = [
  { id: 'home', label: 'Home' },
  { id: 'inbox', label: 'Inbox' },
  { id: 'tests', label: 'Tests' },
  { id: 'context', label: 'Context' },
  { id: 'policies', label: 'Policies' },
  { id: 'demo', label: 'Demo' },
  { id: 'settings', label: 'Settings' },
];

export function Nav({ active, onTab, onDemo, candidateCount, entityCount }: NavProps) {
  const buttons = useRef<Array<HTMLButtonElement | null>>([]);
  const choose = (tab: Tab) => {
    if (tab === 'demo') {
      onDemo();
      return;
    }
    onTab(tab);
  };
  return (
    <nav
      aria-label="Основная навигация"
      role="tablist"
      className="nav-tabs"
      style={{
        display: 'flex',
      }}
    >
      {TABS.map((t, index) => (
        <button
          key={t.id}
          ref={(button) => {
            buttons.current[index] = button;
          }}
          role="tab"
          aria-selected={active === t.id}
          aria-controls="main-panel"
          tabIndex={active === t.id ? 0 : -1}
          onClick={() => choose(t.id)}
          onKeyDown={(event) => {
            const next = tabIndexForKey(index, TABS.length, event.key);
            if (next === null) return;
            const nextTab = TABS[next];
            if (!nextTab) return;
            event.preventDefault();
            choose(nextTab.id);
            buttons.current[next]?.focus();
          }}
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
            <span
              style={{
                position: 'absolute',
                top: '-4px',
                right: '-4px',
                background: '#ef4444',
                color: '#fff',
                borderRadius: '50%',
                width: '18px',
                height: '18px',
                fontSize: '11px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
            >
              {candidateCount}
            </span>
          )}
        </button>
      ))}
      <span className="nav-count">{entityCount} entities</span>
    </nav>
  );
}
