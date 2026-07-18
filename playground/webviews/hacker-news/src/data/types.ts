export type TagId = 'core' | 'rfc' | 'showcase' | 'tauri' | 'electron' | 'cli' | 'help';

export type Sort = 'hot' | 'new' | 'top';

export type Variant = 'dense' | 'cards';

export interface Post {
  id: number;
  title: string;
  /** Source domain, when the post links out. */
  url?: string;
  tag: TagId;
  /** Base score before client-side votes are applied. */
  base: number;
  author: string;
  /** Age in hours (kept static so server + client render identically). */
  age: number;
  comments: number;
  body: string;
}

export interface CommentNode {
  id: string;
  author: string;
  base: number;
  age: number;
  body: string;
  /** Marks the original poster. */
  op?: boolean;
  children?: CommentNode[];
}

export interface User {
  bio: string;
  karma: string;
  joined: string;
}
