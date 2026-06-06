import { existsSync } from 'node:fs';
import path from 'node:path';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { remote } from 'webdriverio';
import { type AppiumServer, ensureAppiumDrivers, startAppiumServer } from './appium.js';
import { type AndroidDevice, ensureAndroidDevice } from './device.js';

const PKG_DIR = path.join(import.meta.dirname, '..');
const APP_PACKAGE = 'dev.wvb.testapp';
const APP_ACTIVITY = 'dev.wvb.testapp.MainActivity';
const AVD = process.env.ANDROID_AVD ?? 'Pixel_5_API_31';

interface NativeTestResult {
  name: string;
  passed: boolean;
  error?: string;
}

/**
 * Parses the `tv_output` TextView, whose lines look like:
 *   `✓ <name>`            — a passing test
 *   `✗ <name>`            — a failing test
 *   `  <error message>`   — the (indented) error for the preceding failure
 */
function parseOutput(output: string): NativeTestResult[] {
  const results: NativeTestResult[] = [];
  const lines = output.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    if (line.startsWith('✓ ')) {
      results.push({ name: line.slice(2).trim(), passed: true });
    } else if (line.startsWith('✗ ')) {
      const name = line.slice(2).trim();
      const next = lines[i + 1];
      let error: string | undefined;
      if (next?.startsWith('  ') && !/^[✓✗]/.test(next.trimStart())) {
        error = next.trim();
        i++;
      }
      results.push({ name, passed: false, error });
    }
  }
  return results;
}

let server: AppiumServer | undefined;
let device: AndroidDevice | undefined;
let driver: Awaited<ReturnType<typeof remote>> | undefined;

beforeAll(async () => {
  const apkPath = path.join(
    PKG_DIR,
    'android',
    'testapp',
    'build',
    'outputs',
    'apk',
    'debug',
    'testapp-debug.apk'
  );
  if (!existsSync(apkPath)) {
    throw new Error(
      `APK not found: ${apkPath}\nRun \`yarn build-ffi-android\` (or \`yarn test-ffi-android\`) first.`
    );
  }

  await ensureAppiumDrivers(['uiautomator2']);
  device = await ensureAndroidDevice(AVD);
  server = await startAppiumServer();

  driver = await remote({
    hostname: '127.0.0.1',
    port: server.port,
    path: '/',
    logLevel: 'error',
    capabilities: {
      platformName: 'Android',
      'appium:automationName': 'UiAutomator2',
      'appium:udid': device.udid,
      'appium:app': apkPath,
      'appium:appPackage': APP_PACKAGE,
      'appium:appActivity': APP_ACTIVITY,
      'appium:autoGrantPermissions': true,
      'appium:fullReset': false,
      'appium:newCommandTimeout': 180,
    },
  });
}, 1_800_000);

afterAll(async () => {
  if (driver) {
    await driver.deleteSession().catch(() => {});
  }
  await server?.stop();
  if (device?.bootedByUs && !process.env.WVB_E2E_KEEP) {
    await device.shutdown();
  }
});

test('android FFI native suite passes', async () => {
  const byId = (id: string) =>
    driver!.$(`android=new UiSelector().resourceId("${APP_PACKAGE}:id/${id}")`);

  const runButton = await byId('btn_run');
  await runButton.waitForDisplayed({ timeout: 30_000 });
  await runButton.click();

  const summaryEl = await byId('tv_summary');
  let summary = '';

  await driver!.waitUntil(
    async () => {
      summary = await summaryEl.getText();
      return /(\d+)\s+passed,\s+(\d+)\s+failed/.test(summary);
    },
    { timeout: 120_000, interval: 1000, timeoutMsg: 'tests did not complete in time' }
  );

  const output = (await (await byId('tv_output')).getText()).trim();
  const results = parseOutput(output);

  expect(
    results.length,
    'native test suite produced no results — did the app crash?'
  ).toBeGreaterThan(0);
  const failures = results.filter(r => !r.passed);
  if (failures.length > 0) {
    const detail = failures
      .map(f => `  ✗ ${f.name}${f.error ? `\n      ${f.error}` : ''}`)
      .join('\n');
    expect.fail(`${failures.length} of ${results.length} native tests failed:\n${detail}`);
  }
});
