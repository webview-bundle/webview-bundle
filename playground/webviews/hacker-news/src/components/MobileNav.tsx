import { Link, useRouterState } from '@tanstack/react-router';
import { CURRENT_USER } from '../data';
import { cn } from '../lib/cn';

const itemBase =
  'flex flex-1 flex-col items-center gap-[3px] py-[9px] pb-[11px] text-[10px] transition-colors';

export function MobileNav() {
  const pathname = useRouterState({ select: s => s.location.pathname });
  const isFeed = pathname === '/';
  const isProfile = pathname.startsWith('/u/');

  return (
    <nav className="flex flex-shrink-0 border-t border-border-1 bg-bg-1 pb-[env(safe-area-inset-bottom)] lg:hidden">
      <Link
        to="/"
        search={prev => ({ ...prev, tag: undefined, q: undefined, compose: undefined })}
        className={cn(itemBase, isFeed ? 'text-accent' : 'text-fg-4')}
      >
        <span className="text-[16px] leading-none">⌂</span>home
      </Link>
      <Link
        to="/"
        search={prev => ({ ...prev, compose: undefined })}
        className={cn(itemBase, 'text-fg-4')}
      >
        <span className="text-[15px] leading-none">⌕</span>search
      </Link>
      <Link
        to="/"
        search={prev => ({ ...prev, compose: true })}
        className={cn(itemBase, 'text-fg-4')}
      >
        <span className="text-[18px] leading-[0.8] text-accent">⊕</span>submit
      </Link>
      <Link
        to="/u/$username"
        params={{ username: CURRENT_USER }}
        className={cn(itemBase, isProfile ? 'text-accent' : 'text-fg-4')}
      >
        <span className="text-[15px] leading-none">◉</span>profile
      </Link>
    </nav>
  );
}
