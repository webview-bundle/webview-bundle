import type { WebviewDriver } from '@wvb-playground/testdriver';
import { test } from 'vitest';
import { testCases } from './cases';

/** Resolves the driver each test runs against (typically created in a `beforeAll`). */
export type DriverProvider = () => WebviewDriver | Promise<WebviewDriver>;

/**
 * Register every testbed case as a vitest `test`. Call it inside a `describe` in
 * an e2e spec that owns the native host and its driver:
 *
 * ```ts
 * describe('testbed', () => {
 *   let driver: WebviewDriver;
 *   beforeAll(async () => {
 *     driver = createPlaywrightDriver(window, { baseURL: 'app://testbed.wvb' });
 *   });
 *   defineTestbedSuite(() => driver);
 * });
 * ```
 */
export function defineTestbedSuite(getDriver: DriverProvider): void {
  for (const testCase of testCases) {
    test(testCase.name, async () => {
      await testCase.run(await getDriver());
    });
  }
}
