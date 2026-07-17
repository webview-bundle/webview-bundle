import { Link } from '@tanstack/react-router';
import type { FlatComment } from '../data';
import { ageLabel, flattenComments } from '../data';
import { cn } from '../lib/cn';
import { useAppState, useVote } from '../lib/store';

export function CommentTree() {
  const { collapsed, toggleCollapse } = useAppState();
  const list = flattenComments(collapsed);
  return (
    <div>
      {list.map(c => (
        <CommentItem
          key={c.id}
          c={c}
          collapsed={collapsed.has(c.id)}
          onToggle={() => toggleCollapse(c.id)}
        />
      ))}
    </div>
  );
}

function CommentItem({
  c,
  collapsed,
  onToggle,
}: {
  c: FlatComment;
  collapsed: boolean;
  onToggle: () => void;
}) {
  const v = useVote(`c:${c.id}`, c.base);
  return (
    <div
      data-testid="comment"
      style={{
        marginLeft: c.depth * 16,
        paddingLeft: c.depth > 0 ? 12 : 0,
        borderLeft: c.depth > 0 ? '1px solid var(--border-1)' : 'none',
      }}
    >
      <div className="flex items-center gap-2 py-[6px] pb-px text-[12px]">
        <button
          type="button"
          data-testid="comment-toggle"
          onClick={onToggle}
          aria-label={collapsed ? 'Expand thread' : 'Collapse thread'}
          aria-expanded={!collapsed}
          className="inline-block w-[18px] cursor-pointer text-left text-fg-4 transition-colors hover:text-accent"
        >
          {collapsed ? '[+]' : '[–]'}
        </button>
        <Link
          to="/u/$username"
          params={{ username: c.author }}
          data-testid="author-link"
          className="font-semibold text-fg-1 transition-colors hover:text-accent"
        >
          {c.author}
        </Link>
        {c.op && (
          <span className="rounded-[3px] border border-border-1 bg-accent-subtle px-[5px] text-[9.5px] text-accent">
            OP
          </span>
        )}
        <span className="text-fg-4">
          {v.score} pts · {ageLabel(c.age)}
        </span>
        {collapsed && c.childCount > 0 && (
          <span className="text-fg-4">· +{c.childCount} hidden</span>
        )}
      </div>
      {!collapsed && (
        <>
          <div className="pt-0.5 pb-1.5 pl-[26px] text-[13px] leading-[1.55] text-fg-2">
            {c.body}
          </div>
          <div className="flex items-center gap-3 pb-2 pl-[26px] text-[11px] text-fg-4">
            <button
              type="button"
              onClick={v.up}
              aria-label="Upvote comment"
              className={cn('cursor-pointer transition-colors', v.dir === 1 && 'text-accent')}
            >
              ▲
            </button>
            <button
              type="button"
              onClick={v.down}
              aria-label="Downvote comment"
              className={cn('cursor-pointer transition-colors', v.dir === -1 && 'text-downvote')}
            >
              ▼
            </button>
            <span className="cursor-pointer transition-colors hover:text-accent">reply</span>
          </div>
        </>
      )}
    </div>
  );
}
