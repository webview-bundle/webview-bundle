import { By, error, type WebDriver, type WebElement } from 'selenium-webdriver';
import type { DriverOptions, WaitOptions, WebviewDriver } from './driver';
import { joinUrl } from './internal';

/**
 * `el.isDisplayed()`/`el.click()`/… reject with this when the DOM mutated (SSR
 * hydration, client-side route changes) between resolving an element and using
 * it, invalidating the handle.
 */
function isStaleElementError(err: unknown): boolean {
  return (
    err instanceof error.StaleElementReferenceError ||
    (err as { name?: string } | null)?.name === 'StaleElementReferenceError'
  );
}

/**
 * A {@link WebviewDriver} backed by a Selenium `WebDriver` — for Tauri via
 * `tauri-driver` (which speaks the W3C WebDriver protocol). `selenium-webdriver`
 * is an optional peer dependency.
 *
 * ```ts
 * import { Builder } from 'selenium-webdriver';
 * import { createSeleniumDriver } from '@wvb-playground/testing/selenium';
 *
 * const wd = await new Builder().usingServer('http://localhost:4444').build();
 * const driver = createSeleniumDriver(wd, { baseURL: 'tauri://localhost' });
 * ```
 */
export function createSeleniumDriver(wd: WebDriver, options: DriverOptions): WebviewDriver {
  const { baseURL, defaultTimeoutMs = 10_000 } = options;
  const timeout = (opts?: WaitOptions) => opts?.timeoutMs ?? defaultTimeoutMs;

  async function firstVisible(selector: string): Promise<WebElement | null> {
    for (const el of await wd.findElements(By.css(selector))) {
      try {
        if (await el.isDisplayed()) return el;
      } catch (err) {
        // The element vanished between findElements() and isDisplayed() (e.g. a
        // re-render). Treat it as "not visible yet" so the surrounding wd.wait()
        // re-queries a fresh snapshot instead of rejecting outright.
        if (!isStaleElementError(err)) throw err;
      }
    }
    return null;
  }

  async function requireVisible(selector: string, timeoutMs: number): Promise<WebElement> {
    const el = await wd.wait(
      () => firstVisible(selector),
      timeoutMs,
      `element never became visible: ${selector}`
    );
    // `wait` only resolves on a truthy value, so `el` is non-null here.
    if (!el) throw new Error(`element never became visible: ${selector}`);
    return el;
  }

  // Resolve a visible element and run `action` against it, retrying when the
  // element goes stale in the window between resolution and use — so a re-render
  // racing the action doesn't fail the whole assertion (mirrors Playwright's
  // auto-retrying locators).
  async function withVisible<T>(
    selector: string,
    action: (el: WebElement) => Promise<T>,
    timeoutMs: number
  ): Promise<T> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const el = await requireVisible(selector, Math.max(0, deadline - Date.now()));
      try {
        return await action(el);
      } catch (err) {
        if (!isStaleElementError(err) || Date.now() >= deadline) throw err;
      }
    }
  }

  return {
    async goto(path) {
      await wd.get(joinUrl(baseURL, path));
    },
    location: () => wd.executeScript<string>('return location.pathname + location.search;'),
    async click(selector) {
      await withVisible(selector, el => el.click(), defaultTimeoutMs);
    },
    async fill(selector, value) {
      await withVisible(
        selector,
        async el => {
          await el.clear();
          await el.sendKeys(value);
        },
        defaultTimeoutMs
      );
    },
    async text(selector) {
      return withVisible(selector, async el => (await el.getText()).trim(), defaultTimeoutMs);
    },
    async getAttribute(selector, name) {
      const el = await firstVisible(selector);
      return el ? el.getAttribute(name) : null;
    },
    async count(selector) {
      return (await wd.findElements(By.css(selector))).length;
    },
    async isVisible(selector) {
      return (await firstVisible(selector)) !== null;
    },
    async waitForVisible(selector, opts) {
      await requireVisible(selector, timeout(opts));
    },
    async waitForHidden(selector, opts) {
      await wd.wait(
        async () => (await firstVisible(selector)) === null,
        timeout(opts),
        `element never hid: ${selector}`
      );
    },
  };
}
