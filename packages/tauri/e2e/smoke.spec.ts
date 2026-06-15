import { type ChildProcess, spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { createSeleniumDriver } from '@wvb-playground/testing/selenium';
import { testCases } from '@wvb-playground/webview-hacker-news/testing';
import { Builder, type WebDriver } from 'selenium-webdriver';
import { afterAll, beforeAll, describe, test } from 'vitest';

async function waitForPort(port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const open = await new Promise<boolean>(resolve => {
      const socket = net.connect(port, '127.0.0.1');
      socket.once('connect', () => {
        socket.destroy();
        resolve(true);
      });
      socket.once('error', () => resolve(false));
    });
    if (open) {
      return;
    }
    await delay(300);
  }
  throw new Error(`tauri-driver did not start listening on port ${port} within ${timeoutMs}ms`);
}

// NOTE: tauri-driver only supports Linux and Windows — this suite cannot run on macOS.
// It expects `tauri-driver` (cargo install tauri-driver) and, on Linux, `WebKitWebDriver`
// (webkit2gtk) to be on PATH. The fixture app binary is built automatically by `global-setup.ts`
// (`cargo build --release`) before the suite runs.
function isDriverSupported(): boolean {
  return process.platform === 'linux' || process.platform === 'win32';
}

// The fixture app registers the `bundle://` protocol against the committed builtin bundles
// (see fixtures/app/bundles) and opens a window at `bundle://hacker-news.wvb`. Each shared,
// platform-agnostic case from `@wvb-playground/webview-hacker-news/testing` then drives that window
// through a Selenium-backed `WebviewDriver` (over `tauri-driver`), asserting the Hacker News demo
// behaves the same served through the bundle protocol as on every other platform.
describe('smoke', { skip: !isDriverSupported() }, () => {
  const dirname = path.dirname(fileURLToPath(import.meta.url));
  const appDir = path.join(dirname, 'fixtures', 'app');
  const bundlesDir = path.join(appDir, 'bundles');

  const binaryName = process.platform === 'win32' ? 'wvb-tauri-e2e-app.exe' : 'wvb-tauri-e2e-app';
  const binaryPath = path.join(appDir, 'src-tauri', 'target', 'release', binaryName);

  let tauriDriver: ChildProcess | undefined;
  let driver: WebDriver | undefined;

  beforeAll(async () => {
    if (!existsSync(binaryPath)) {
      throw new Error(
        `Tauri app binary not found: ${binaryPath}\nIt is normally built by global-setup.ts; run \`cargo build --release\` in fixtures/app/src-tauri to build it manually.`
      );
    }

    // Point the app at the committed bundle fixture (e2e/fixtures/app/bundles). The env var must be
    // set before tauri-driver spawns so the launched app inherits it.
    process.env.WVB_E2E_BUNDLES_DIR = bundlesDir;

    tauriDriver = spawn('tauri-driver', [], { stdio: [null, 'inherit', 'inherit'] });
    await waitForPort(4444, 30_000); // wait until tauri-driver is accepting connections

    driver = await new Builder()
      .withCapabilities({
        browserName: 'wry',
        'tauri:options': { application: binaryPath },
      })
      .usingServer('http://127.0.0.1:4444/')
      .build();
  }, 180_000);

  afterAll(async () => {
    try {
      await driver?.quit();
    } catch (e) {
      console.error('WebDriver quit failed', e);
    }
    if (tauriDriver != null) {
      tauriDriver.kill('SIGTERM');
      await delay(1000);
      if (tauriDriver.exitCode == null) {
        tauriDriver.kill('SIGKILL');
      }
    }
  });

  for (const testCase of testCases) {
    test(testCase.name, async () => {
      if (driver == null) {
        throw new Error('WebDriver session was not created');
      }
      const webviewDriver = createSeleniumDriver(driver, {
        baseURL: 'bundle://hacker-news.wvb',
        defaultTimeoutMs: 30_000,
      });
      await testCase.run(webviewDriver);
    }, 60_000);
  }
});
