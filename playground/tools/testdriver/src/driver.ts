export interface WaitOptions {
  /** Maximum time to wait, in milliseconds. */
  timeoutMs?: number;
}

/**
 * The minimal, platform-agnostic surface that drives a webview during E2E tests.
 * Test suites are written against this interface; a per-platform implementation
 * (see `@wvb-playground/testing/{playwright,selenium,appium}`) maps it onto a
 * concrete automation tool:
 *
 *  - Electron  → Playwright (`page.locator(...)`)
 *  - Tauri     → Selenium WebDriver (`driver.findElement(By.css(...))`)
 *  - Android   → Appium / WebdriverIO in the WEBVIEW context (`$(...)`)
 *  - iOS       → Appium / WebdriverIO in the WEBVIEW context (`$(...)`)
 *
 * Selector semantics
 * ------------------
 * `selector` is a CSS selector evaluated against the live webview DOM. All
 * single-element operations (`click`, `fill`, `text`, `getAttribute`,
 * `waitForVisible`) act on the **first visible match**. This lets a test id that
 * appears in both the desktop and the mobile chrome (e.g. a theme toggle or a
 * navigation link) resolve to whichever layout is currently on screen, so suites
 * stay viewport-agnostic.
 *
 * Failure semantics
 * -----------------
 * Operations should reject if the target cannot be found/actioned within the
 * host's timeout. The wait helpers reject on timeout. Rejected promises fail the
 * surrounding test.
 */
export interface WebviewDriver {
  /**
   * Load the app at an in-app path with a full document load (resetting client
   * state), e.g. `"/"`, `"/post/3"`, `"/u/byte_poet"`. The implementation resolves
   * the path against a configured base URL (an `app://`/`tauri://` scheme, a dev
   * server, a `file://` bundle, …).
   */
  goto(path: string): Promise<void>;

  /** The current in-app location as `pathname + search`, e.g. `"/post/3?tag=core"`. */
  location(): Promise<string>;

  /** Click the first visible element matching `selector`. */
  click(selector: string): Promise<void>;

  /** Replace the value of the first visible `<input>`/`<textarea>` matching `selector`. */
  fill(selector: string, value: string): Promise<void>;

  /** Trimmed text content of the first visible element matching `selector`. */
  text(selector: string): Promise<string>;

  /** Value of `name` on the first visible element matching `selector`, or `null`. */
  getAttribute(selector: string, name: string): Promise<string | null>;

  /** Number of elements in the DOM matching `selector`. */
  count(selector: string): Promise<number>;

  /** Whether at least one element matching `selector` is visible. */
  isVisible(selector: string): Promise<boolean>;

  /** Resolve once an element matching `selector` becomes visible; reject on timeout. */
  waitForVisible(selector: string, options?: WaitOptions): Promise<void>;

  /** Resolve once no element matching `selector` is visible; reject on timeout. */
  waitForHidden(selector: string, options?: WaitOptions): Promise<void>;
}

/** Common options shared by the per-platform driver factories. */
export interface DriverOptions {
  /**
   * Base URL that in-app paths passed to {@link WebviewDriver.goto} are resolved
   * against, e.g. `"http://localhost:4173"` or `"app://news.wvb.dev"`.
   */
  baseURL: string;
  /** Default timeout (ms) for waits when a call doesn't pass one. Defaults to 10000. */
  defaultTimeoutMs?: number;
}
