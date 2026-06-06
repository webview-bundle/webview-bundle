import path from 'node:path';
import { type ElectronApplication, _electron as electron } from '@playwright/test';
import { afterEach, beforeEach, expect, test } from 'vitest';

const appMain = path.join(import.meta.dirname, 'fixtures', 'app', 'main.cjs');

let app: ElectronApplication | undefined;

beforeEach(async () => {
  app = await electron.launch({ args: [appMain] });
});

afterEach(async () => {
  try {
    await app?.close();
  } catch (e) {
    console.error('failed to close electron app', e);
  } finally {
    app = undefined;
  }
});

test('serves the Next.js SSG bundle through the app:// bundle protocol', async () => {
  const window = await app!.firstWindow();
  await window.waitForLoadState('domcontentloaded');

  // The bundle's index.html was served by the protocol and rendered by the webview.
  const heading = window.locator('h1', { hasText: 'Pagination with SSG' });
  await heading.waitFor({ state: 'visible', timeout: 30_000 });
  expect(await heading.isVisible()).toBe(true);

  // Next.js rewrites the document title during hydration, so matching it also proves the JS
  // chunks were served through the protocol and executed (the app is interactive, not just HTML).
  await window.waitForFunction(() => document.title.includes('Pagination with SSG'), undefined, {
    timeout: 30_000,
  });
  expect(await window.title()).toMatch(/Pagination with SSG/);

  // A non-HTML sub-resource is also served through the protocol with a correct content type,
  // proving the webview can load bundle assets (not just the entry document).
  const asset = await window.evaluate(async () => {
    const res = await fetch('/build.png');
    return { status: res.status, contentType: res.headers.get('content-type') ?? '' };
  });
  expect(asset.status).toBe(200);
  expect(asset.contentType).toMatch(/image\//);
});

test('navigates between bundle pages via in-app links', async () => {
  const window = await app!.firstWindow();
  await window.waitForLoadState('domcontentloaded');
  const heading = window.locator('h1', { hasText: 'Pagination with SSG' });
  await heading.waitFor({ state: 'visible', timeout: 30_000 });
  expect(await heading.isVisible()).toBe(true);

  // index -> /category: in-app navigation to another page served by the bundle protocol.
  await window.locator('a[href="/category"]').first().click();
  await window.waitForURL(/\/category\/?$/, { timeout: 30_000 });
  expect(window.url()).toMatch(/\/category\/?$/);

  // The category listing and its pagination controls render after navigating.
  const nextPageLink = window.locator('a[href="/category/2/"]').first();
  await nextPageLink.waitFor({ state: 'visible', timeout: 30_000 });
  expect(await nextPageLink.isVisible()).toBe(true);

  // /category -> /category/2: paginate to the next page.
  await nextPageLink.click();
  await window.waitForURL(/\/category\/2\/?$/, { timeout: 30_000 });
  expect(window.url()).toMatch(/\/category\/2\/?$/);

  const previousPageLink = window.locator('a[href="/category/1/"]').first();
  await previousPageLink.waitFor({ state: 'visible', timeout: 30_000 });
  expect(await previousPageLink.isVisible()).toBe(true);
});
