import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { execa } from 'execa';
import { glob } from 'tinyglobby';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { remote } from 'webdriverio';
import {
  APPIUM_PORT,
  type AppiumServer,
  ensureAppiumDrivers,
  startAppiumServer,
} from './appium.js';
import { ensureIosSimulator, type IosSimulator } from './device.js';

const BUNDLE_ID = 'dev.wvb.testapp';
const DEVICE_NAME = process.env.IOS_DEVICE ?? 'iPhone 16';

/** Finds the most recently built simulator `TestApp.app` in Xcode's DerivedData. */
async function findSimulatorApp(): Promise<string | undefined> {
  const pattern = path.join(
    os.homedir(),
    'Library',
    'Developer',
    'Xcode',
    'DerivedData',
    'TestApp-*',
    'Build',
    'Products',
    'Debug-iphonesimulator',
    'TestApp.app'
  );
  const matches = await glob(pattern, {
    onlyDirectories: true,
    absolute: true,
    expandDirectories: false,
  });
  if (matches.length === 0) {
    return undefined;
  }
  const withMtime = await Promise.all(
    matches.map(async app => ({ app, mtime: (await fs.stat(app)).mtimeMs }))
  );
  withMtime.sort((a, b) => b.mtime - a.mtime);
  return withMtime[0]!.app;
}

let server: AppiumServer | undefined;
let device: IosSimulator | undefined;
let driver: Awaited<ReturnType<typeof remote>> | undefined;

beforeAll(async () => {
  const appPath = await findSimulatorApp();
  if (!appPath) {
    throw new Error(
      'Simulator build of TestApp.app not found.\nRun `yarn build-ffi-apple` (or `yarn test-ffi-apple`) first.'
    );
  }

  await ensureAppiumDrivers(['xcuitest']);
  device = await ensureIosSimulator(DEVICE_NAME);

  // Install the app ourselves and launch it by bundle id. Passing `appium:app` pointing at a
  // `.app` *directory* makes the XCUITest driver fail with `EISDIR` while preparing the app,
  // so we sidestep its app-file handling entirely.
  console.log(`[device] installing ${path.basename(appPath)} -> ${device.udid}`);
  await execa('xcrun', ['simctl', 'install', device.udid, appPath]);

  server = await startAppiumServer(APPIUM_PORT);

  const wdaTimeout = process.env.CI ? 600_000 : 300_000;
  const sessionTimeout = process.env.CI ? 720_000 : 360_000;

  driver = await remote({
    hostname: '127.0.0.1',
    port: server.port,
    path: '/',
    logLevel: 'error',
    // Building/launching WebDriverAgent on the first session can take several minutes, well
    // past WDIO's default 120s request timeout. Give session creation room and don't retry.
    connectionRetryTimeout: sessionTimeout,
    connectionRetryCount: 0,
    capabilities: {
      platformName: 'iOS',
      'appium:automationName': 'XCUITest',
      'appium:udid': device.udid,
      'appium:deviceName': device.name,
      'appium:bundleId': BUNDLE_ID,
      'appium:newCommandTimeout': 180,
      'appium:wdaLaunchTimeout': wdaTimeout,
      'appium:wdaConnectionTimeout': wdaTimeout,
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

test('apple FFI native suite passes', async () => {
  let ran = false;
  let passed = 0;
  let failedCount = 0;
  let staticTexts: string[] = [];

  const runButton = await driver!.$('~Run');
  await runButton.waitForDisplayed({ timeout: 120_000 });
  await runButton.click();

  // The summary header renders a "<n> passed" StaticText (and a "<n> failed" one only when
  // there are failures). Poll until the passed count appears, collecting both.
  await driver!.waitUntil(
    async () => {
      const texts = await driver!.$$('-ios predicate string:type == "XCUIElementTypeStaticText"');
      const values: string[] = [];
      let sawPassed = false;
      for (const el of texts) {
        const value = (await el.getText()) ?? '';
        values.push(value);
        const passedMatch = value.match(/^(\d+)\s+passed$/);
        const failedMatch = value.match(/^(\d+)\s+failed$/);
        if (passedMatch) {
          passed = Number(passedMatch[1]);
          sawPassed = true;
        }
        if (failedMatch) {
          failedCount = Number(failedMatch[1]);
        }
      }
      if (sawPassed) {
        staticTexts = values;
      }
      return sawPassed;
    },
    { timeout: 180_000, interval: 1500, timeoutMsg: 'iOS tests did not complete in time' }
  );
  ran = true;

  expect(ran, 'native test suite did not report a summary — did the app crash?').toBe(true);
  if (failedCount > 0) {
    const visible = staticTexts.map(t => `  ${t}`).join('\n');
    expect.fail(`iOS e2e: ${passed} passed, ${failedCount} failed\nVisible on screen:\n${visible}`);
  }
});
