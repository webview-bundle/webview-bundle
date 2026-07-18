/**
 * The `data-testid` contract the Hacker News demo app must satisfy. These ids
 * are the single source of truth shared between the app's markup and the test
 * cases here — keep them in sync with `webviews/hacker-news`.
 */
export const TESTID = {
  /** App shell; also carries `data-theme="light" | "dark"`. */
  appShell: 'app-shell',
  /** Feed view container. */
  feed: 'feed',
  /** Post-detail view container. */
  postDetail: 'post-detail',
  /** Post-detail title (`<h1>`). */
  postDetailTitle: 'post-detail-title',
  /** Profile view container. */
  profile: 'profile',
  /** Profile username heading. */
  profileUsername: 'profile-username',
  /** A single post in the feed (one per post). */
  postRow: 'post-row',
  /** Clickable post title that opens the post detail. */
  postLink: 'post-link',
  /** Vote score in a vote column. */
  voteScore: 'vote-score',
  /** Upvote button in a vote column. */
  upvote: 'upvote',
  /** Downvote button in a vote column. */
  downvote: 'downvote',
  /** Feed sort tabs. */
  sortHot: 'sort-hot',
  sortNew: 'sort-new',
  sortTop: 'sort-top',
  /** The "N posts[ in #tag]" result label. */
  resultCount: 'result-count',
  /** A single comment node. */
  comment: 'comment',
  /** Collapse/expand toggle on a comment. */
  commentToggle: 'comment-toggle',
  /** A link to a user's profile (author of a post or comment). */
  authorLink: 'author-link',
  /** Back-to-feed control (desktop link / mobile button). */
  back: 'back',
  /** Light/dark theme toggle. */
  themeToggle: 'theme-toggle',
  /** Search input. */
  searchInput: 'search-input',
} as const;

/** Build a CSS attribute selector for a `data-testid`. */
export function byTestId(id: string): string {
  return `[data-testid="${id}"]`;
}

/** Selector for a community filter link/chip, e.g. `community("core")`. */
export function community(tag: string): string {
  return byTestId(`community-${tag}`);
}

/** Ready-made CSS selectors for every entry in {@link TESTID}. */
export const sel = {
  appShell: byTestId(TESTID.appShell),
  feed: byTestId(TESTID.feed),
  postDetail: byTestId(TESTID.postDetail),
  postDetailTitle: byTestId(TESTID.postDetailTitle),
  profile: byTestId(TESTID.profile),
  profileUsername: byTestId(TESTID.profileUsername),
  postRow: byTestId(TESTID.postRow),
  postLink: byTestId(TESTID.postLink),
  voteScore: byTestId(TESTID.voteScore),
  upvote: byTestId(TESTID.upvote),
  downvote: byTestId(TESTID.downvote),
  sortHot: byTestId(TESTID.sortHot),
  sortNew: byTestId(TESTID.sortNew),
  sortTop: byTestId(TESTID.sortTop),
  resultCount: byTestId(TESTID.resultCount),
  comment: byTestId(TESTID.comment),
  commentToggle: byTestId(TESTID.commentToggle),
  authorLink: byTestId(TESTID.authorLink),
  back: byTestId(TESTID.back),
  themeToggle: byTestId(TESTID.themeToggle),
  searchInput: byTestId(TESTID.searchInput),
} as const;
