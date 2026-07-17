import { createFileRoute, Link } from '@tanstack/react-router';
import { useState } from 'react';
import { ScrollArea } from '../components/ScrollArea';
import { VoteColumn } from '../components/VoteColumn';
import {
  type AuthorComment,
  ageLabel,
  commentsByAuthor,
  getUser,
  monogram,
  postsByAuthor,
} from '../data';
import type { Post } from '../data/types';
import { cn } from '../lib/cn';
import { useVote } from '../lib/store';

export const Route = createFileRoute('/u/$username')({
  component: Profile,
});

type Tab = 'posts' | 'comments';

function Profile() {
  const { username } = Route.useParams();
  const user = getUser(username);
  const profPosts = postsByAuthor(username);
  const profComments = commentsByAuthor(username);
  const [tab, setTab] = useState<Tab>('posts');

  return (
    <ScrollArea data-testid="profile">
      <div className="mx-auto max-w-[740px] p-4">
        <Link
          to="/"
          data-testid="back"
          className="mb-3.5 hidden text-[12px] text-fg-3 transition-colors hover:text-accent lg:inline-flex"
        >
          ← back to feed
        </Link>

        <header className="flex items-center gap-4 border-b border-border-1 pb-4">
          <div className="flex h-14 w-14 flex-shrink-0 items-center justify-center rounded-xl border border-border-1 bg-accent-subtle font-sans text-[19px] font-bold text-accent">
            {monogram(username)}
          </div>
          <div className="min-w-0">
            <div
              data-testid="profile-username"
              className="font-sans text-[18px] font-bold text-fg-1"
            >
              {username}
            </div>
            <div className="mt-[3px] text-[12px] leading-[1.5] text-fg-3">
              {user?.bio ?? 'webview-bundle community member.'}
            </div>
            <div className="mt-2 flex flex-wrap gap-3.5 text-[12px] text-fg-2">
              <span>
                <b className="text-fg-1">{user?.karma ?? '—'}</b> karma
              </span>
              <span>
                <b className="text-fg-1">{profPosts.length}</b> posts
              </span>
              <span>joined {user?.joined ?? 'recently'}</span>
            </div>
          </div>
        </header>

        <div className="my-3.5 flex w-fit gap-[3px] rounded-lg border border-border-1 bg-bg-2 p-[3px]">
          <TabBtn label="posts" active={tab === 'posts'} onClick={() => setTab('posts')} />
          <TabBtn label="comments" active={tab === 'comments'} onClick={() => setTab('comments')} />
        </div>

        {tab === 'posts' ? (
          profPosts.length > 0 ? (
            profPosts.map(p => <ProfilePostRow key={p.id} post={p} />)
          ) : (
            <Empty label="no posts yet" />
          )
        ) : profComments.length > 0 ? (
          profComments.map(c => <ProfileComment key={c.id} c={c} />)
        ) : (
          <Empty label="no comments yet" />
        )}
      </div>
    </ScrollArea>
  );
}

function TabBtn({
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

function ProfilePostRow({ post }: { post: Post }) {
  const v = useVote(`p:${post.id}`, post.base);
  return (
    <div className="flex items-start gap-2.5 border-b border-border-1 px-0.5 py-[9px]">
      <div className="min-w-[24px]">
        <VoteColumn dir={v.dir} score={v.score} onUp={v.up} onDown={v.down} />
      </div>
      <div className="min-w-0 flex-1">
        <Link
          to="/post/$postId"
          params={{ postId: String(post.id) }}
          className="font-sans text-[14px] font-semibold text-fg-1 transition-colors hover:text-accent"
        >
          {post.title}
        </Link>
        <div className="mt-1 text-[11.5px] text-fg-3">
          #{post.tag} · {v.score} pts · {ageLabel(post.age)} · {post.comments} comments
        </div>
      </div>
    </div>
  );
}

function ProfileComment({ c }: { c: AuthorComment }) {
  return (
    <div className="border-b border-border-1 py-2.5">
      <div className="text-[11px] text-fg-4">
        commented · {c.score} pts · {ageLabel(c.age)}
      </div>
      <div className="mt-1.5 text-[13px] leading-[1.55] text-fg-2">{c.body}</div>
    </div>
  );
}

function Empty({ label }: { label: string }) {
  return <div className="py-12 text-center text-[13px] text-fg-3">{label}</div>;
}
