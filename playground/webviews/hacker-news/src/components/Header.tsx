import { Link, useNavigate, useRouterState } from '@tanstack/react-router';
import { useState } from 'react';
import { CURRENT_USER, monogram } from '../data';
import { useAppState } from '../lib/store';

const avatarClass =
  'flex h-8 w-8 items-center justify-center rounded-md border border-border-1 bg-accent-subtle font-sans text-[12px] font-bold text-accent';

function ThemeButton({ className }: { className?: string }) {
  const { theme, toggleTheme } = useAppState();
  return (
    <button
      type="button"
      data-testid="theme-toggle"
      onClick={toggleTheme}
      aria-label={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
      className={`flex items-center justify-center rounded-md border border-border-1 bg-bg-2 text-fg-2 transition-colors hover:bg-bg-3 hover:text-fg-1 ${className ?? ''}`}
    >
      {theme === 'dark' ? '☀' : '☾'}
    </button>
  );
}

export function Header() {
  const pathname = useRouterState({ select: s => s.location.pathname });
  const search = useRouterState({ select: s => s.location.search as { q?: string } });
  const navigate = useNavigate();

  const isFeed = pathname === '/';
  const mobileTitle = pathname.startsWith('/post')
    ? 'thread'
    : pathname.startsWith('/u/')
      ? decodeURIComponent(pathname.slice(3))
      : 'WEBVIEW BUNDLE';

  const [q, setQ] = useState(search.q ?? '');
  const onSearch = (value: string) => {
    setQ(value);
    navigate({ to: '/', search: prev => ({ ...prev, q: value || undefined }) });
  };

  return (
    <header className="z-10 flex h-[calc(52px+env(safe-area-inset-top))] flex-shrink-0 items-center border-b border-border-1 bg-bg-1 px-3 pt-[env(safe-area-inset-top)] lg:px-4">
      {/* ---------- desktop ---------- */}
      <div className="hidden w-full items-center gap-3.5 lg:flex">
        <Link to="/" className="flex items-baseline gap-[7px]">
          <span className="font-sans text-[14px] font-bold tracking-[0.04em] text-fg-1">
            WEBVIEW BUNDLE
          </span>
          <span className="text-[12px] text-fg-3">{'// news'}</span>
        </Link>

        <div className="flex flex-1 justify-center">
          <label className="relative w-[min(440px,100%)]">
            <span className="-translate-y-1/2 pointer-events-none absolute top-1/2 left-2.5 text-[12px] text-fg-4">
              ⌕
            </span>
            <input
              value={q}
              onChange={e => onSearch(e.target.value)}
              placeholder="search posts, tags, users…"
              aria-label="Search"
              data-testid="search-input"
              className="w-full rounded-md border border-border-2 bg-bg-2 py-[7px] pr-2.5 pl-7 text-[13px] text-fg-1 outline-none transition focus:border-accent focus:ring-[3px] focus:ring-accent/15"
            />
          </label>
        </div>

        <ThemeButton className="h-8 w-8 text-[14px]" />
        <Link
          to="/"
          search={prev => ({ ...prev, compose: true })}
          className="flex h-8 items-center gap-1.5 rounded-md bg-accent px-3.5 text-[13px] font-medium text-white transition-colors hover:bg-accent-hover"
        >
          + submit
        </Link>
        <Link
          to="/u/$username"
          params={{ username: CURRENT_USER }}
          className={avatarClass}
          aria-label="Your profile"
        >
          {monogram(CURRENT_USER)}
        </Link>
      </div>

      {/* ---------- mobile ---------- */}
      <div className="flex w-full items-center gap-2.5 lg:hidden">
        {isFeed ? (
          <Link to="/" className="flex items-center gap-2">
            <span className="font-sans text-[13px] font-bold tracking-[0.04em] text-fg-1">
              WEBVIEW BUNDLE
            </span>
          </Link>
        ) : (
          <>
            <button
              type="button"
              data-testid="back"
              onClick={() => navigate({ to: '/' })}
              aria-label="Back to feed"
              className="flex h-[30px] w-[30px] items-center justify-center rounded-md border border-border-1 bg-bg-2 text-[16px] text-fg-2"
            >
              ←
            </button>
            <span className="truncate font-semibold text-[13px] text-fg-1">{mobileTitle}</span>
          </>
        )}
        <ThemeButton className="ml-auto h-[30px] w-[30px] text-[13px]" />
        <Link
          to="/u/$username"
          params={{ username: CURRENT_USER }}
          className="flex h-[30px] w-[30px] items-center justify-center rounded-md border border-border-1 bg-accent-subtle font-sans text-[11px] font-bold text-accent"
          aria-label="Your profile"
        >
          {monogram(CURRENT_USER)}
        </Link>
      </div>
    </header>
  );
}
