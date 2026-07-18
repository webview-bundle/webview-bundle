import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { Composer } from '../components/Composer';
import { PostCard, PostRow } from '../components/PostRow';
import { RightRail } from '../components/RightRail';
import { ScrollArea } from '../components/ScrollArea';
import { selectFeed, TAGS } from '../data';
import type { Sort, TagId, Variant } from '../data/types';
import { cn } from '../lib/cn';

interface FeedSearch {
  tag?: TagId;
  q?: string;
  sort?: Sort;
  compose?: boolean;
}

const SORTS: Sort[] = ['hot', 'new', 'top'];

export const Route = createFileRoute('/')({
  validateSearch: (search: Record<string, unknown>): FeedSearch => {
    const tag = TAGS.includes(search.tag as TagId) ? (search.tag as TagId) : undefined;
    const sort = SORTS.includes(search.sort as Sort) ? (search.sort as Sort) : undefined;
    const q = typeof search.q === 'string' && search.q.trim() ? search.q : undefined;
    const compose = search.compose === true || search.compose === 'true' ? true : undefined;
    return { tag, sort, q, compose };
  },
  component: Feed,
});

function Feed() {
  const { tag, q, sort = 'hot', compose } = Route.useSearch();
  const navigate = useNavigate();
  const [variant, setVariant] = useState<Variant>('dense');

  const posts = selectFeed({ tag, q, sort });
  const resultLabel = `${posts.length} ${posts.length === 1 ? 'post' : 'posts'}${tag ? ` in #${tag}` : ''}`;

  const setSort = (s: Sort) =>
    navigate({ to: '/', search: prev => ({ ...prev, sort: s === 'hot' ? undefined : s }) });

  return (
    <>
      <ScrollArea data-testid="feed">
        <MobileFeedControls activeTag={tag} q={q} />

        <div className="mx-auto max-w-[740px] px-4 pt-3.5 pb-8">
          {compose && <Composer />}

          <div className="mb-3 flex items-center gap-2">
            <div className="flex gap-[3px] rounded-lg border border-border-1 bg-bg-2 p-[3px]">
              {SORTS.map(s => (
                <SortTab key={s} label={s} active={sort === s} onClick={() => setSort(s)} />
              ))}
            </div>
            <span data-testid="result-count" className="ml-auto text-[12px] text-fg-3">
              {resultLabel}
            </span>
            <ViewToggle variant={variant} onChange={setVariant} />
          </div>

          {posts.length === 0 ? (
            <div className="py-16 text-center text-[13px] text-fg-3">
              no posts match — try another tag or query.
            </div>
          ) : variant === 'dense' ? (
            posts.map((p, i) => <PostRow key={p.id} post={p} rank={i + 1} />)
          ) : (
            posts.map(p => <PostCard key={p.id} post={p} />)
          )}
        </div>
      </ScrollArea>
      <RightRail />
    </>
  );
}

function SortTab({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      data-testid={`sort-${label}`}
      onClick={onClick}
      className={cn(
        'rounded-md px-3 py-[5px] text-[12px] transition',
        active ? 'bg-bg-1 font-bold text-fg-1 shadow-sm' : 'font-medium text-fg-3 hover:text-fg-1'
      )}
    >
      {label}
    </button>
  );
}

function ViewToggle({ variant, onChange }: { variant: Variant; onChange: (v: Variant) => void }) {
  return (
    <div className="hidden gap-[3px] rounded-lg border border-border-1 bg-bg-2 p-[3px] sm:flex">
      {(['dense', 'cards'] as const).map(v => (
        <button
          key={v}
          type="button"
          onClick={() => onChange(v)}
          className={cn(
            'rounded-md px-2.5 py-[5px] text-[11px] transition',
            variant === v
              ? 'bg-bg-1 font-semibold text-fg-1 shadow-sm'
              : 'text-fg-3 hover:text-fg-1'
          )}
        >
          {v}
        </button>
      ))}
    </div>
  );
}

const chip = 'flex-shrink-0 whitespace-nowrap rounded-full border px-2.5 py-[5px] text-[12px]';
const chipActive = 'border-accent bg-accent-subtle text-accent';
const chipIdle = 'border-border-1 bg-bg-2 text-fg-2';

function MobileFeedControls({ activeTag, q }: { activeTag?: TagId; q?: string }) {
  const navigate = useNavigate();
  const [value, setValue] = useState(q ?? '');

  return (
    <div className="sticky top-0 z-[2] bg-bg-1 px-3 pt-2.5 pb-1 lg:hidden">
      <label className="relative mb-2 block">
        <span className="-translate-y-1/2 pointer-events-none absolute top-1/2 left-2.5 text-[12px] text-fg-4">
          ⌕
        </span>
        <input
          value={value}
          onChange={e => {
            setValue(e.target.value);
            navigate({ to: '/', search: prev => ({ ...prev, q: e.target.value || undefined }) });
          }}
          placeholder="search…"
          aria-label="Search"
          data-testid="search-input"
          className="w-full rounded-md border border-border-2 bg-bg-2 py-[7px] pr-2.5 pl-7 text-[13px] text-fg-1 outline-none transition focus:border-accent focus:ring-[3px] focus:ring-accent/15"
        />
      </label>
      <div className="no-scrollbar flex gap-1.5 overflow-x-auto pb-1">
        <Link
          to="/"
          search={prev => ({ ...prev, tag: undefined })}
          className={cn(chip, !activeTag ? chipActive : chipIdle)}
        >
          all
        </Link>
        {TAGS.map(t => (
          <Link
            key={t}
            to="/"
            search={prev => ({ ...prev, tag: t })}
            data-testid={`community-${t}`}
            className={cn(chip, activeTag === t ? chipActive : chipIdle)}
          >
            #{t}
          </Link>
        ))}
      </div>
    </div>
  );
}
