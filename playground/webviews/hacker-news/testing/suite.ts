import type { WebviewDriver } from '@wvb-playground/testdriver';
import { test } from 'vitest';
import { testCases } from './cases';

/** Resolves the driver each test runs against (typically created in a `beforeAll`). */
export type DriverProvider = () => WebviewDriver | Promise<WebviewDriver>;

/**
 * Register every Hacker News case as a vitest `test`. Call it inside a `describe`
 * in an e2e spec that owns the native host and its driver:
 *
 * ```ts
 * describe('smoke', () => {
 *   let app: ElectronApplication;
 *   beforeAll(async () => {
 *     app = await electron.launch(...);
 *   });
 *   defineHackerNewsSuite(async () =>
 *     createPlaywrightDriver(await app.firstWindow(), { baseURL: 'app://hacker-news.wvb' })
 *   );
 * });
 * ```
 */
export function defineHackerNewsSuite(getDriver: DriverProvider): void {
  for (const testCase of testCases) {
    test(testCase.name, async () => {
      await testCase.run(await getDriver());
    });
  }
}
