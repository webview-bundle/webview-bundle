import type { Page } from 'playwright-core';
import type { DriverOptions, WaitOptions, WebviewDriver } from './driver';
import { joinUrl } from './internal';

/**
 * A {@link WebviewDriver} backed by a Playwright `Page` — for Electron (via the
 * `_electron` API, passing `app.firstWindow()`) or any Chromium/WebKit/Firefox
 * page. `playwright-core` is an optional peer dependency.
 *
 * ```ts
 * import { _electron as electron } from '@playwright/test';
 * import { createPlaywrightDriver } from '@wvb-playground/testing/playwright';
 *
 * const app = await electron.launch({ args: [main] });
 * const driver = createPlaywrightDriver(await app.firstWindow(), { baseURL: 'app://news.wvb.dev' });
 * ```
 */
export function createPlaywrightDriver(page: Page, options: DriverOptions): WebviewDriver {
  const { baseURL, defaultTimeoutMs } = options;
  // `:visible` resolves the first *visible* match — important when a test id
  // exists in both the desktop and mobile chrome.
  const visible = (selector: string) => page.locator(`${selector}:visible`).first();
  const timeout = (opts?: WaitOptions) => opts?.timeoutMs ?? defaultTimeoutMs;

  return {
    async goto(path) {
      await page.goto(joinUrl(baseURL, path));
    },
    location: () => page.evaluate(() => location.pathname + location.search),
    async click(selector) {
      await visible(selector).click({ timeout: defaultTimeoutMs });
    },
    async fill(selector, value) {
      await visible(selector).fill(value, { timeout: defaultTimeoutMs });
    },
    async text(selector) {
      const value = await visible(selector).innerText({ timeout: defaultTimeoutMs });
      return value.trim();
    },
    getAttribute: (selector, name) =>
      visible(selector).getAttribute(name, { timeout: defaultTimeoutMs }),
    count: selector => page.locator(selector).count(),
    async isVisible(selector) {
      return (await page.locator(`${selector}:visible`).count()) > 0;
    },
    async waitForVisible(selector, opts) {
      await visible(selector).waitFor({ state: 'visible', timeout: timeout(opts) });
    },
    async waitForHidden(selector, opts) {
      await page
        .locator(selector)
        .first()
        .waitFor({ state: 'hidden', timeout: timeout(opts) });
    },
  };
}
