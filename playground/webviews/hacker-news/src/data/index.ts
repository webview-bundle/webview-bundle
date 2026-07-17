import { comments } from './comments';
import { posts, TAGS } from './posts';
import type { CommentNode, Post, Sort, TagId, User } from './types';
import { users } from './users';

export type { CommentNode, Post, Sort, TagId, User, Variant } from './types';
export { comments, posts, TAGS, users };

/** The signed-in user (the "CD" avatar in the header). */
export const CURRENT_USER = 'core_dev';

/** Format an age in hours as `Nh` / `Nd`. */
export function ageLabel(hours: number): string {
  return hours < 24 ? `${hours}h` : `${Math.floor(hours / 24)}d`;
}

/** Number of posts carrying each community tag. */
export function tagCount(tag: TagId): number {
  return posts.filter(p => p.tag === tag).length;
}

export interface FeedQuery {
  tag?: TagId | null;
  q?: string;
  sort?: Sort;
}

export function selectFeed({ tag, q, sort = 'hot' }: FeedQuery): Post[] {
  let list = posts.slice();
  if (tag) list = list.filter(p => p.tag === tag);
  const needle = (q ?? '').trim().toLowerCase();
  if (needle) {
    list = list.filter(
      p =>
        p.title.toLowerCase().includes(needle) ||
        p.tag.includes(needle) ||
        p.author.toLowerCase().includes(needle)
    );
  }
  if (sort === 'new') list.sort((a, b) => a.age - b.age);
  else if (sort === 'top') list.sort((a, b) => b.base - a.base);
  else list.sort((a, b) => hotScore(b) - hotScore(a));
  return list;
}

function hotScore(p: Post): number {
  return p.base / (p.age + 2) ** 0.35;
}

export function getPost(id: number): Post | undefined {
  return posts.find(p => p.id === id);
}

export function getUser(name: string): User | undefined {
  return users[name];
}

export function postsByAuthor(name: string): Post[] {
  return posts.filter(p => p.author === name);
}

export interface AuthorComment {
  id: string;
  body: string;
  score: number;
  age: number;
}

export function commentsByAuthor(name: string): AuthorComment[] {
  const out: AuthorComment[] = [];
  const walk = (nodes: CommentNode[]) => {
    for (const n of nodes) {
      if (n.author === name) out.push({ id: n.id, body: n.body, score: n.base, age: n.age });
      if (n.children) walk(n.children);
    }
  };
  walk(comments);
  return out;
}

/** All distinct authors across posts + comments — used to prerender profiles. */
export function allAuthors(): string[] {
  const set = new Set<string>();
  for (const p of posts) set.add(p.author);
  const walk = (nodes: CommentNode[]) => {
    for (const n of nodes) {
      set.add(n.author);
      if (n.children) walk(n.children);
    }
  };
  walk(comments);
  return [...set];
}

export function countDescendants(node: CommentNode): number {
  if (!node.children) return 0;
  return node.children.reduce((acc, c) => acc + 1 + countDescendants(c), 0);
}

export function totalCommentCount(): number {
  let n = 0;
  const walk = (nodes: CommentNode[]) => {
    for (const c of nodes) {
      n++;
      if (c.children) walk(c.children);
    }
  };
  walk(comments);
  return n;
}

export interface FlatComment {
  id: string;
  depth: number;
  author: string;
  op: boolean;
  body: string;
  base: number;
  age: number;
  childCount: number;
}

/**
 * Flatten the comment tree to a render list, honoring a set of collapsed ids:
 * a collapsed node is shown but its descendants are omitted.
 */
export function flattenComments(collapsed: Set<string>): FlatComment[] {
  const out: FlatComment[] = [];
  const walk = (nodes: CommentNode[], depth: number) => {
    for (const n of nodes) {
      const isCollapsed = collapsed.has(n.id);
      out.push({
        id: n.id,
        depth,
        author: n.author,
        op: !!n.op,
        body: n.body,
        base: n.base,
        age: n.age,
        childCount: countDescendants(n),
      });
      if (!isCollapsed && n.children) walk(n.children, depth + 1);
    }
  };
  walk(comments, 0);
  return out;
}

export function monogram(user: string): string {
  const parts = user.split('_');
  const a = parts[0]?.[0] ?? '';
  const b = (parts[1] ?? parts[0] ?? '')[0] ?? '';
  return (a + b).toUpperCase();
}
