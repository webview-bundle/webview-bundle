import { Link, useRouterState } from '@tanstack/react-router';
import { TAGS, tagCount } from '../data';
import { cn } from '../lib/cn';

const rowBase =
  'flex w-full items-center justify-between gap-2 rounded-md border border-transparent px-2 py-1.5 text-left text-[12.5px] transition-colors';
const activeRow = 'bg-accent-subtle font-semibold text-accent';
const idleRow = 'text-fg-2 hover:bg-bg-2';

export function LeftSidebar() {
  const pathname = useRouterState({ select: s => s.location.pathname });
  const tag = useRouterState({ select: s => (s.location.search as { tag?: string }).tag });
  const sort = useRouterState({ select: s => (s.location.search as { sort?: string }).sort });

  const onFeed = pathname === '/';
  const activeTag = onFeed ? tag : undefined;
  const homeActive = onFeed && !activeTag && sort !== 'top';
  const popActive = onFeed && !activeTag && sort === 'top';

  return (
    <aside className="hidden w-[210px] flex-shrink-0 flex-col gap-[18px] overflow-y-auto border-r border-border-1 bg-bg-1 px-2 py-3.5 lg:flex">
      <nav>
        <div className="px-2 pb-1.5 text-[10.5px] tracking-[0.08em] text-fg-4">FEEDS</div>
        <Link to="/" search={{}} className={cn(rowBase, homeActive ? activeRow : idleRow)}>
          ⌂ home
        </Link>
        <Link
          to="/"
          search={{ sort: 'top' }}
          className={cn(rowBase, popActive ? activeRow : idleRow)}
        >
          ✦ popular
        </Link>
        <button type="button" className={cn(rowBase, idleRow, 'cursor-pointer')}>
          ▣ saved
        </button>
      </nav>

      <nav>
        <div className="px-2 pb-1.5 text-[10.5px] tracking-[0.08em] text-fg-4">COMMUNITIES</div>
        {TAGS.map(t => (
          <Link
            key={t}
            to="/"
            search={{ tag: t }}
            data-testid={`community-${t}`}
            className={cn(rowBase, activeTag === t ? activeRow : idleRow)}
          >
            <span>#{t}</span>
            <span className="text-[11px] text-fg-4">{tagCount(t)}</span>
          </Link>
        ))}
      </nav>

      <div className="mt-auto border-t border-border-1 px-2 pt-2.5 pb-1 text-[11px] leading-[1.7] text-fg-4">
        @wvb/web · v1.4.0
        <br />
        app://news.wvb.dev
      </div>
    </aside>
  );
}
