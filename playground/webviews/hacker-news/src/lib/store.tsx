import { createContext, type ReactNode, useCallback, useContext, useEffect, useState } from 'react';

export type Theme = 'light' | 'dark';
export type VoteDir = 1 | -1 | 0;

const THEME_KEY = 'wvb-theme';

interface AppState {
  theme: Theme;
  toggleTheme: () => void;
  votes: Record<string, VoteDir>;
  vote: (key: string, dir: 1 | -1) => void;
  collapsed: Set<string>;
  toggleCollapse: (id: string) => void;
}

const AppStateContext = createContext<AppState | null>(null);

export function AppStateProvider({ children }: { children: ReactNode }) {
  // Server and the first client render are always `light`, so prerendered HTML
  // hydrates without a mismatch; the stored preference is applied in an effect.
  const [theme, setTheme] = useState<Theme>('light');
  const [votes, setVotes] = useState<Record<string, VoteDir>>({});
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());

  useEffect(() => {
    try {
      const saved = localStorage.getItem(THEME_KEY);
      if (saved === 'dark' || saved === 'light') setTheme(saved);
    } catch {
      /* localStorage unavailable (e.g. sandboxed webview) — keep default */
    }
  }, []);

  const toggleTheme = useCallback(() => {
    setTheme(prev => {
      const next = prev === 'dark' ? 'light' : 'dark';
      try {
        localStorage.setItem(THEME_KEY, next);
      } catch {
        /* ignore */
      }
      return next;
    });
  }, []);

  const vote = useCallback((key: string, dir: 1 | -1) => {
    setVotes(prev => ({ ...prev, [key]: prev[key] === dir ? 0 : dir }));
  }, []);

  const toggleCollapse = useCallback((id: string) => {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  return (
    <AppStateContext.Provider
      value={{ theme, toggleTheme, votes, vote, collapsed, toggleCollapse }}
    >
      {children}
    </AppStateContext.Provider>
  );
}

export function useAppState(): AppState {
  const ctx = useContext(AppStateContext);
  if (!ctx) throw new Error('useAppState must be used within <AppStateProvider>');
  return ctx;
}

export interface VoteState {
  dir: VoteDir;
  score: number;
  up: () => void;
  down: () => void;
}

/** Resolve the current vote direction + adjusted score for a votable item. */
export function useVote(key: string, base: number): VoteState {
  const { votes, vote } = useAppState();
  const dir = votes[key] ?? 0;
  return {
    dir,
    score: base + dir,
    up: () => vote(key, 1),
    down: () => vote(key, -1),
  };
}
