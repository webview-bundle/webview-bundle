import type { WebviewDriver } from '@wvb-playground/testdriver';
import { assert, assertContains, assertEqual, assertGreaterThan } from './assert';
import { community, sel } from './selectors';

/**
 * Expected values baked into the demo's fixed data (`webviews/hacker-news`). If
 * the demo's seed data changes, update these.
 */
export const EXPECTED = {
  totalPosts: 12,
  /** Under the "top" sort, the highest-scored post is first. */
  topPost: { score: 530, titleIncludes: 'Tauri integration merged' },
  /** Filtering by this community yields this many posts. */
  community: { tag: 'core', posts: 3 },
  deepLink: {
    post: { path: '/post/7', titleIncludes: 'magic number' },
    profile: { path: '/u/byte_poet', username: 'byte_poet' },
  },
} as const;

async function score(driver: WebviewDriver): Promise<number> {
  return Number(await driver.text(sel.voteScore));
}

/** One platform-agnostic E2E scenario, expressed against a {@link WebviewDriver}. */
export interface TestCase {
  /** Stable, human-readable name (used as the vitest test title). */
  name: string;
  /** Runs the scenario; throws (e.g. an AssertionError) on failure. */
  run(driver: WebviewDriver): Promise<void>;
}

/**
 * The full suite of scenarios that verify the Hacker News demo's flows work end
 * to end inside a webview. Each case is self-contained and starts with a
 * `goto(...)` so it can run in any order against a shared session.
 */
export const testCases: TestCase[] = [
  {
    name: 'feed renders the full list of posts',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.feed);
      await driver.waitForVisible(sel.postRow);
      assertEqual(
        await driver.count(sel.postRow),
        EXPECTED.totalPosts,
        'number of posts on the feed'
      );
      assertContains((await driver.text(sel.resultCount)).toLowerCase(), 'posts', 'result label');
    },
  },
  {
    name: 'sorting by "top" puts the highest-scored post first',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.feed);
      await driver.click(sel.sortTop);
      assertContains(await driver.location(), 'sort=top', 'sort reflected in the URL');
      await driver.waitForVisible(sel.postRow);
      assertEqual(await score(driver), EXPECTED.topPost.score, 'highest score is first');
      assertContains(
        await driver.text(sel.postLink),
        EXPECTED.topPost.titleIncludes,
        'highest-scored post is first'
      );
    },
  },
  {
    name: 'opening a post shows its detail and comments',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      const title = await driver.text(sel.postLink);
      await driver.click(sel.postLink);
      await driver.waitForVisible(sel.postDetail);
      assert((await driver.location()).startsWith('/post/'), 'navigated to a /post/ URL');
      assertEqual(await driver.text(sel.postDetailTitle), title, 'detail shows the clicked post');
      assertGreaterThan(await driver.count(sel.comment), 0, 'post has comments');
    },
  },
  {
    name: 'upvoting a post increments then restores its score',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      const before = await score(driver);
      await driver.click(sel.upvote);
      assertEqual(await score(driver), before + 1, 'score after upvoting');
      await driver.click(sel.upvote);
      assertEqual(await score(driver), before, 'score after toggling the upvote off');
    },
  },
  {
    name: 'collapsing a comment thread hides its replies',
    run: async driver => {
      await driver.goto('/post/3');
      await driver.waitForVisible(sel.postDetail);
      await driver.waitForVisible(sel.comment);
      const before = await driver.count(sel.comment);
      await driver.click(sel.commentToggle);
      const collapsed = await driver.count(sel.comment);
      assert(
        collapsed < before,
        `collapsing should hide replies (before=${before}, after=${collapsed})`
      );
      await driver.click(sel.commentToggle);
      assertEqual(await driver.count(sel.comment), before, 'expanding restores the replies');
    },
  },
  {
    name: 'clicking an author opens their profile',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      await driver.click(sel.authorLink);
      await driver.waitForVisible(sel.profile);
      assert((await driver.location()).startsWith('/u/'), 'navigated to a /u/ URL');
      assertGreaterThan(
        (await driver.text(sel.profileUsername)).length,
        0,
        'profile shows a username'
      );
    },
  },
  {
    name: 'filtering by a community narrows the feed',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      assertEqual(await driver.count(sel.postRow), EXPECTED.totalPosts, 'unfiltered post count');
      await driver.click(community(EXPECTED.community.tag));
      await driver.waitForVisible(sel.feed);
      assertContains(
        await driver.location(),
        `tag=${EXPECTED.community.tag}`,
        'tag reflected in the URL'
      );
      assertEqual(await driver.count(sel.postRow), EXPECTED.community.posts, 'filtered post count');
      assertContains(
        (await driver.text(sel.resultCount)).toLowerCase(),
        EXPECTED.community.tag,
        'result label names the community'
      );
    },
  },
  {
    name: 'toggling the theme switches to dark mode',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.appShell);
      assertEqual(
        await driver.getAttribute(sel.appShell, 'data-theme'),
        'light',
        'starts in light theme'
      );
      await driver.click(sel.themeToggle);
      assertEqual(
        await driver.getAttribute(sel.appShell, 'data-theme'),
        'dark',
        'switches to dark theme'
      );
    },
  },
  {
    name: 'going back returns from a post to the feed',
    run: async driver => {
      await driver.goto('/post/3');
      await driver.waitForVisible(sel.postDetail);
      await driver.click(sel.back);
      await driver.waitForVisible(sel.feed);
      assertEqual(await driver.location(), '/', 'back returns to the feed');
    },
  },
  {
    name: 'every route type is served as a deep link (SSG)',
    run: async driver => {
      // Feed.
      await driver.goto('/');
      await driver.waitForVisible(sel.feed);

      // A post detail loaded directly (not via in-app navigation).
      await driver.goto(EXPECTED.deepLink.post.path);
      await driver.waitForVisible(sel.postDetail);
      assertContains(
        await driver.text(sel.postDetailTitle),
        EXPECTED.deepLink.post.titleIncludes,
        'deep-linked post renders'
      );

      // A profile loaded directly.
      await driver.goto(EXPECTED.deepLink.profile.path);
      await driver.waitForVisible(sel.profile);
      assertContains(
        await driver.text(sel.profileUsername),
        EXPECTED.deepLink.profile.username,
        'deep-linked profile renders'
      );
    },
  },
];
