import type { Browser } from 'webdriverio';
import type { DriverOptions, WaitOptions, WebviewDriver } from './driver';
import { joinUrl } from './internal';

/**
 * A {@link WebviewDriver} backed by an Appium session driven through WebdriverIO,
 * for Android and iOS. The `browser` must already be switched into the WEBVIEW
 * context (`browser.switchContext('WEBVIEW_…')`) so CSS selectors hit the DOM.
 * `webdriverio` is an optional peer dependency.
 *
 * ```ts
 * import { remote } from 'webdriverio';
 * import { createAppiumDriver } from '@wvb-playground/testing/appium';
 *
 * const browser = await remote({ capabilities: { ... } });
 * // … switch into the webview context …
 * const driver = createAppiumDriver(browser, { baseURL: 'app://news.wvb.dev' });
 * ```
 */
export function createAppiumDriver(browser: Browser, options: DriverOptions): WebviewDriver {
  const { baseURL, defaultTimeoutMs = 10_000 } = options;
  const timeout = (opts?: WaitOptions) => opts?.timeoutMs ?? defaultTimeoutMs;

  async function firstVisible(selector: string): Promise<WebdriverIO.Element | null> {
    for (const el of await browser.$$(selector)) {
      if (await el.isDisplayed()) return el;
    }
    return null;
  }

  async function requireVisible(selector: string, timeoutMs: number): Promise<WebdriverIO.Element> {
    let found: WebdriverIO.Element | null = null;
    await browser.waitUntil(
      async () => {
        found = await firstVisible(selector);
        return found !== null;
      },
      { timeout: timeoutMs, timeoutMsg: `element never became visible: ${selector}` }
    );
    if (!found) throw new Error(`element never became visible: ${selector}`);
    return found;
  }

  return {
    async goto(path) {
      await browser.url(joinUrl(baseURL, path));
    },
    location: () => browser.execute(() => location.pathname + location.search),
    async click(selector) {
      await (await requireVisible(selector, defaultTimeoutMs)).click();
    },
    async fill(selector, value) {
      const el = await requireVisible(selector, defaultTimeoutMs);
      await el.clearValue();
      await el.setValue(value);
    },
    async text(selector) {
      return (await (await requireVisible(selector, defaultTimeoutMs)).getText()).trim();
    },
    async getAttribute(selector, name) {
      const el = await firstVisible(selector);
      return el ? el.getAttribute(name) : null;
    },
    async count(selector) {
      return (await browser.$$(selector)).length;
    },
    async isVisible(selector) {
      return (await firstVisible(selector)) !== null;
    },
    async waitForVisible(selector, opts) {
      await requireVisible(selector, timeout(opts));
    },
    async waitForHidden(selector, opts) {
      await browser.waitUntil(async () => (await firstVisible(selector)) === null, {
        timeout: timeout(opts),
        timeoutMsg: `element never hid: ${selector}`,
      });
    },
  };
}
