import { Link } from '@tanstack/react-router';
import { ageLabel } from '../data';
import type { Post } from '../data/types';
import { useVote } from '../lib/store';
import { TagBadge } from './TagBadge';
import { VoteColumn } from './VoteColumn';

/** Compact "dense" feed row — the default, matching the primary screenshots. */
export function PostRow({ post, rank }: { post: Post; rank: number }) {
  const v = useVote(`p:${post.id}`, post.base);
  const postId = String(post.id);
  return (
    <article
      data-testid="post-row"
      className="flex items-start gap-2.5 border-b border-border-1 px-1 py-[9px]"
    >
      <span className="min-w-[18px] pt-[3px] text-right text-[11px] text-fg-4 tabular-nums">
        {rank.toString().padStart(2, '0')}
      </span>
      <div className="min-w-[24px]">
        <VoteColumn dir={v.dir} score={v.score} onUp={v.up} onDown={v.down} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-baseline gap-[7px]">
          <Link
            to="/post/$postId"
            params={{ postId }}
            data-testid="post-link"
            className="font-sans text-[14px] font-semibold tracking-[-0.01em] text-fg-1 transition-colors hover:text-accent"
          >
            {post.title}
          </Link>
          {post.url && <span className="text-[11px] text-fg-4">({post.url})</span>}
        </div>
        <div className="mt-[5px] flex flex-wrap items-center gap-2 text-[11.5px] text-fg-3">
          <TagBadge tag={post.tag} />
          <span>{v.score} pts</span>
          <span>
            by{' '}
            <Link
              to="/u/$username"
              params={{ username: post.author }}
              data-testid="author-link"
              className="text-fg-2 transition-colors hover:text-accent"
            >
              {post.author}
            </Link>
          </span>
          <span>{ageLabel(post.age)}</span>
          <Link
            to="/post/$postId"
            params={{ postId }}
            className="transition-colors hover:text-accent"
          >
            ✦ {post.comments} comments
          </Link>
        </div>
      </div>
    </article>
  );
}

/** Roomier "cards" feed item — toggled via the feed view switcher. */
export function PostCard({ post }: { post: Post }) {
  const v = useVote(`p:${post.id}`, post.base);
  const postId = String(post.id);
  return (
    <article
      data-testid="post-row"
      className="mb-2.5 flex overflow-hidden rounded-lg border border-border-1 bg-bg-1 transition-colors hover:border-border-2"
    >
      <div className="flex min-w-[46px] flex-col items-center justify-start border-r border-border-1 bg-bg-2 px-2 py-2.5">
        <VoteColumn dir={v.dir} score={v.score} onUp={v.up} onDown={v.down} />
      </div>
      <div className="min-w-0 flex-1 px-3.5 py-[11px]">
        <div className="mb-[7px] flex flex-wrap items-center gap-2 text-[11px] text-fg-3">
          <TagBadge tag={post.tag} />
          <span>
            posted by{' '}
            <Link
              to="/u/$username"
              params={{ username: post.author }}
              data-testid="author-link"
              className="text-fg-2 transition-colors hover:text-accent"
            >
              {post.author}
            </Link>{' '}
            · {ageLabel(post.age)}
          </span>
        </div>
        <Link
          to="/post/$postId"
          params={{ postId }}
          data-testid="post-link"
          className="block font-sans text-[15.5px] font-semibold leading-[1.3] tracking-[-0.01em] text-fg-1 transition-colors hover:text-accent"
        >
          {post.title}
        </Link>
        {post.url && <div className="mt-1.5 text-[11px] text-accent">→ {post.url}</div>}
        <p className="mt-2 line-clamp-2 text-[12px] leading-[1.5] text-fg-2">{post.body}</p>
        <div className="mt-[11px] flex gap-4 text-[12px] text-fg-3">
          <Link
            to="/post/$postId"
            params={{ postId }}
            className="transition-colors hover:text-accent"
          >
            ▭ {post.comments} comments
          </Link>
          <span className="cursor-pointer transition-colors hover:text-accent">↗ share</span>
          <span className="cursor-pointer transition-colors hover:text-accent">✦ save</span>
        </div>
      </div>
    </article>
  );
}
