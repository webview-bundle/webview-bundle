import { type ChildProcess, spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import { Builder, By, until, type WebDriver } from 'selenium-webdriver';
import { afterAll, beforeAll, describe, expect, test } from 'vitest';

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
// (webkit2gtk) to be on PATH, and the app binary to be built (see the `e2e` script).
function isDriverSupported(): boolean {
  return process.platform === 'linux' || process.platform === 'win32';
}

describe('bundle protocol', { skip: !isDriverSupported() }, () => {
  const dirname = path.dirname(fileURLToPath(import.meta.url));
  const appDir = path.join(dirname, 'fixtures', 'app');
  const bundlesDir = path.join(appDir, 'bundles');
  const indexUrl = 'bundle://next.wvb';

  const binaryName = process.platform === 'win32' ? 'wvb-tauri-e2e-app.exe' : 'wvb-tauri-e2e-app';
  const binaryPath = path.join(appDir, 'src-tauri', 'target', 'release', binaryName);

  let tauriDriver: ChildProcess | undefined;
  let driver: WebDriver | undefined;

  beforeAll(async () => {
    if (!existsSync(binaryPath)) {
      throw new Error(
        `Tauri app binary not found: ${binaryPath}\nBuild it first with \`yarn e2e\` (or \`yarn e2e-build\`).`
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

  test('serves the Next.js SSG bundle through the bundle:// protocol', async () => {
    if (driver == null) {
      throw new Error('WebDriver session was not created');
    }

    const heading = await driver.wait(until.elementLocated(By.css('h1')), 30_000);
    await driver.wait(until.elementTextContains(heading, 'Pagination with SSG'), 30_000);
    expect(await heading.getText()).toContain('Pagination with SSG');
    expect(await driver.getTitle()).toContain('Pagination with SSG');
  }, 60_000);

  test('navigates between bundle pages via in-app links', async () => {
    if (driver == null) {
      throw new Error('WebDriver session was not created');
    }

    await driver.get(indexUrl);
    await driver.wait(until.elementLocated(By.css('a[href="/category"]')), 30_000);

    await (await driver.findElement(By.css('a[href="/category"]'))).click();
    await driver.wait(until.urlContains('/category'), 30_000);
    await driver.wait(until.elementLocated(By.css('a[href="/category/2/"]')), 30_000);

    await (await driver.findElement(By.css('a[href="/category/2/"]'))).click();
    await driver.wait(until.urlContains('/category/2'), 30_000);
    await driver.wait(until.elementLocated(By.css('a[href="/category/1/"]')), 30_000);
  }, 60_000);
});
