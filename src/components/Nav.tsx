import { useRef } from 'react';
import { selectedTabId, tabIndexForKey, tabStopFor } from '../data/nav';

export type Tab = 'home' | 'inbox' | 'preview' | 'tests' | 'context' | 'policies' | 'settings';

interface NavProps {
  active: Tab;
  onTab: (tab: Tab) => void;
  onDemo: () => void;
  candidateCount: number;
  entityCount: number;
  showPreview: boolean;
}

const TABS: { id: Tab; label: string }[] = [
  { id: 'home', label: 'Home' },
  { id: 'inbox', label: 'Inbox' },
  { id: 'preview', label: 'Preview' },
  { id: 'tests', label: 'Tests' },
  { id: 'context', label: 'Context' },
  { id: 'policies', label: 'Policies' },
  { id: 'settings', label: 'Settings' },
];

export function Nav({ active, onTab, onDemo, candidateCount, entityCount, showPreview }: NavProps) {
  const buttons = useRef<Array<HTMLButtonElement | null>>([]);
  const tabs = showPreview ? TABS : TABS.filter((tab) => tab.id !== 'preview');
  const selected = selectedTabId(
    active,
    tabs.map((tab) => tab.id),
  );
  return (
    <nav aria-label="Основная навигация" className="nav-tabs">
      <div role="tablist" aria-label="SOUL sections" className="nav-tablist">
        {tabs.map((t, index) => (
          <button
            key={t.id}
            ref={(button) => {
              buttons.current[index] = button;
            }}
            role="tab"
            id={`nav-tab-${t.id}`}
            aria-selected={selected === t.id}
            aria-controls="main-panel"
            tabIndex={tabStopFor(t.id, selected)}
            onClick={() => onTab(t.id)}
            onKeyDown={(event) => {
              const next = tabIndexForKey(index, tabs.length, event.key);
              if (next === null) return;
              const nextTab = tabs[next];
              if (!nextTab) return;
              event.preventDefault();
              onTab(nextTab.id);
              buttons.current[next]?.focus();
            }}
            style={{
              padding: '6px 16px',
              border: 'none',
              borderRadius: '6px',
              background: selected === t.id ? '#6366f1' : 'transparent',
              color: selected === t.id ? '#fff' : '#333',
              cursor: 'pointer',
              fontWeight: selected === t.id ? '600' : '400',
              fontSize: '14px',
              position: 'relative',
            }}
          >
            {t.label}
            {t.id === 'inbox' && candidateCount > 0 && (
              <span
                aria-label={`${candidateCount} candidates to review`}
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
                <span aria-hidden="true">{candidateCount}</span>
              </span>
            )}
          </button>
        ))}
      </div>
      <button type="button" className="nav-demo-link" onClick={onDemo}>
        Demo
      </button>
      <span className="nav-count">{entityCount} entities</span>
    </nav>
  );
}
