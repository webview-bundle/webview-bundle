import { createFileRoute, Link } from '@tanstack/react-router';
import { CommentTree } from '../components/CommentTree';
import { ScrollArea } from '../components/ScrollArea';
import { TagBadge } from '../components/TagBadge';
import { VoteColumn } from '../components/VoteColumn';
import { ageLabel, getPost, totalCommentCount } from '../data';
import type { Post } from '../data/types';
import { useVote } from '../lib/store';

export const Route = createFileRoute('/post/$postId')({
  component: PostDetail,
});

function PostDetail() {
  const { postId } = Route.useParams();
  const post = getPost(Number(postId));

  if (!post) {
    return (
      <ScrollArea data-testid="post-detail">
        <div className="mx-auto max-w-[740px] px-4 pt-3.5 pb-16">
          <Link
            to="/"
            data-testid="back"
            className="mb-3.5 inline-flex text-[12px] text-fg-3 transition-colors hover:text-accent"
          >
            ← back to feed
          </Link>
          <div className="py-16 text-center text-[13px] text-fg-3">
            post <code className="text-fg-2">{postId}</code> not found.
          </div>
        </div>
      </ScrollArea>
    );
  }

  return <PostView post={post} />;
}

function PostView({ post }: { post: Post }) {
  const v = useVote(`p:${post.id}`, post.base);

  return (
    <ScrollArea data-testid="post-detail">
      <div className="mx-auto max-w-[740px] px-4 pt-3.5 pb-16">
        <Link
          to="/"
          data-testid="back"
          className="mb-3.5 hidden text-[12px] text-fg-3 transition-colors hover:text-accent lg:inline-flex"
        >
          ← back to feed
        </Link>

        <div className="flex gap-3 border-b border-border-1 pb-4">
          <div className="min-w-[30px] pt-0.5">
            <VoteColumn big dir={v.dir} score={v.score} onUp={v.up} onDown={v.down} />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2 text-[11.5px] text-fg-3">
              <TagBadge tag={post.tag} />
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
              {post.url && <span className="text-accent">{post.url}</span>}
            </div>
            <h1
              data-testid="post-detail-title"
              className="mt-2 font-sans text-[21px] font-bold leading-[1.25] tracking-[-0.02em] text-fg-1"
            >
              {post.title}
            </h1>
            <p className="mt-3 text-[13.5px] leading-[1.65] text-fg-2">{post.body}</p>
            <div className="mt-3.5 flex gap-4 text-[12px] text-fg-3">
              <span className="text-fg-2">▭ {post.comments} comments</span>
              <span className="cursor-pointer transition-colors hover:text-accent">↗ share</span>
              <span className="cursor-pointer transition-colors hover:text-accent">✦ save</span>
            </div>
          </div>
        </div>

        <div className="mt-4 mb-1.5">
          <textarea
            placeholder="add a comment…"
            aria-label="Add a comment"
            rows={2}
            className="w-full resize-none rounded-lg border border-border-2 bg-bg-2 px-3 py-2.5 text-[13px] text-fg-1 outline-none transition focus:border-accent focus:ring-[3px] focus:ring-accent/15"
          />
        </div>
        <div className="mt-2 mb-2.5 text-[12px] text-fg-3">
          {totalCommentCount()} comments · sorted by best
        </div>

        <CommentTree />
      </div>
    </ScrollArea>
  );
}
