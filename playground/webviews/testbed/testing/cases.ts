import type { WebviewDriver } from '@wvb-playground/testdriver';
import { expect } from 'vitest';
import { METHOD_SPECS, methodSel, sel } from './selectors';

/** One platform-agnostic E2E scenario, expressed against a {@link WebviewDriver}. */
export interface TestCase {
  /** Stable, human-readable name (used as the vitest test title). */
  name: string;
  /** Runs the scenario; a failed `expect` throws and fails the test. */
  run(driver: WebviewDriver): Promise<void>;
}

/**
 * The suite drives the testbed against the **real** `@wvb/bridge` — there is no
 * mock. Because the values a bridge returns depend on the host (which bundles are
 * installed, whether a remote/updater is configured), the cases assert only what
 * is host-independent: the platform is reported, and each method — when run —
 * reaches a terminal, rendered outcome. That outcome is `ok` in a configured
 * native host and `error` (a `BridgeError`) when a namespace is unconfigured or
 * no native host is present, so the same suite is meaningful in every webview.
 *
 * Run it against a native host that loads the testbed bundle (as the Hacker News
 * suite runs against its bundle); {@link ./suite} wires the cases into vitest.
 */
async function open(driver: WebviewDriver): Promise<void> {
  await driver.goto('/');
  await driver.waitForVisible(sel.appShell);
}

/** One generated case per bridge method: run it, assert it renders an outcome. */
const methodCases: TestCase[] = METHOD_SPECS.map(spec => ({
  name: `${spec.id} invokes and renders an outcome`,
  run: async driver => {
    await open(driver);
    await driver.click(methodSel.run(spec.id));
    await driver.waitForVisible(methodSel.result(spec.id));
    expect(await driver.getAttribute(methodSel.result(spec.id), 'data-status')).toBeOneOf([
      'ok',
      'error',
    ]);
    expect((await driver.text(methodSel.result(spec.id))).length).toBeGreaterThan(0);
  },
}));

/**
 * The full suite that exercises every `@wvb/bridge` method through the testbed
 * UI. Each case is self-contained and starts with an `open(...)` so it can run in
 * any order against a shared session.
 */
export const testCases: TestCase[] = [
  {
    name: 'detects and displays the platform',
    run: async driver => {
      await open(driver);
      expect((await driver.text(sel.platformType)).length).toBeGreaterThan(0);
    },
  },
  ...methodCases,
  {
    name: 'raw invoke() calls a command by name and renders an outcome',
    run: async driver => {
      await open(driver);
      await driver.fill(sel.invokeName, 'sourceListBundles');
      await driver.click(sel.invokeRun);
      await driver.waitForVisible(sel.invokeResult);
      expect(await driver.getAttribute(sel.invokeResult, 'data-status')).toBeOneOf(['ok', 'error']);
    },
  },
];
