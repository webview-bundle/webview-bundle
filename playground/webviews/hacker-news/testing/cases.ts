import type { WebviewDriver } from '@wvb-playground/testdriver';
import { expect } from 'vitest';
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
  /** Runs the scenario; a failed `expect` throws and fails the test. */
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
      expect(await driver.count(sel.postRow), 'number of posts on the feed').toBe(
        EXPECTED.totalPosts
      );
      expect((await driver.text(sel.resultCount)).toLowerCase(), 'result label').toContain('posts');
    },
  },
  {
    name: 'sorting by "top" puts the highest-scored post first',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.feed);
      await driver.click(sel.sortTop);
      expect(await driver.location(), 'sort reflected in the URL').toContain('sort=top');
      await driver.waitForVisible(sel.postRow);
      expect(await score(driver), 'highest score is first').toBe(EXPECTED.topPost.score);
      expect(await driver.text(sel.postLink), 'highest-scored post is first').toContain(
        EXPECTED.topPost.titleIncludes
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
      expect((await driver.location()).startsWith('/post/'), 'navigated to a /post/ URL').toBe(
        true
      );
      expect(await driver.text(sel.postDetailTitle), 'detail shows the clicked post').toBe(title);
      expect(await driver.count(sel.comment), 'post has comments').toBeGreaterThan(0);
    },
  },
  {
    name: 'upvoting a post increments then restores its score',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      const before = await score(driver);
      await driver.click(sel.upvote);
      expect(await score(driver), 'score after upvoting').toBe(before + 1);
      await driver.click(sel.upvote);
      expect(await score(driver), 'score after toggling the upvote off').toBe(before);
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
      expect(
        collapsed,
        `collapsing should hide replies (before=${before}, after=${collapsed})`
      ).toBeLessThan(before);
      await driver.click(sel.commentToggle);
      expect(await driver.count(sel.comment), 'expanding restores the replies').toBe(before);
    },
  },
  {
    name: 'clicking an author opens their profile',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      await driver.click(sel.authorLink);
      await driver.waitForVisible(sel.profile);
      expect((await driver.location()).startsWith('/u/'), 'navigated to a /u/ URL').toBe(true);
      expect(
        (await driver.text(sel.profileUsername)).length,
        'profile shows a username'
      ).toBeGreaterThan(0);
    },
  },
  {
    name: 'filtering by a community narrows the feed',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.postRow);
      expect(await driver.count(sel.postRow), 'unfiltered post count').toBe(EXPECTED.totalPosts);
      await driver.click(community(EXPECTED.community.tag));
      await driver.waitForVisible(sel.feed);
      expect(await driver.location(), 'tag reflected in the URL').toContain(
        `tag=${EXPECTED.community.tag}`
      );
      expect(await driver.count(sel.postRow), 'filtered post count').toBe(EXPECTED.community.posts);
      expect(
        (await driver.text(sel.resultCount)).toLowerCase(),
        'result label names the community'
      ).toContain(EXPECTED.community.tag);
    },
  },
  {
    name: 'toggling the theme switches to dark mode',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.appShell);
      expect(await driver.getAttribute(sel.appShell, 'data-theme'), 'starts in light theme').toBe(
        'light'
      );
      await driver.click(sel.themeToggle);
      expect(await driver.getAttribute(sel.appShell, 'data-theme'), 'switches to dark theme').toBe(
        'dark'
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
      expect(await driver.location(), 'back returns to the feed').toBe('/');
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
      expect(await driver.text(sel.postDetailTitle), 'deep-linked post renders').toContain(
        EXPECTED.deepLink.post.titleIncludes
      );

      // A profile loaded directly.
      await driver.goto(EXPECTED.deepLink.profile.path);
      await driver.waitForVisible(sel.profile);
      expect(await driver.text(sel.profileUsername), 'deep-linked profile renders').toContain(
        EXPECTED.deepLink.profile.username
      );
    },
  },
];
