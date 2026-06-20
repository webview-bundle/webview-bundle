import { mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { type ElectronApplication, _electron as electron } from '@playwright/test';
import { createPlaywrightDriver } from '@wvb-playground/testing/playwright';
import { testCases } from '@wvb-playground/webview-hacker-news/testing';
import { afterAll, beforeAll, describe, test } from 'vitest';

const appMain = path.join(import.meta.dirname, 'fixtures', 'app', 'main.cjs');

describe('smoke', () => {
  let app: ElectronApplication | undefined;
  let userDataDir: string | undefined;

  beforeAll(async () => {
    userDataDir = await mkdtemp(path.join(os.tmpdir(), 'wvb-electron-e2e-'));
    app = await electron.launch({ args: [appMain, `--user-data-dir=${userDataDir}`] });
    await app.firstWindow();
  });

  afterAll(async () => {
    try {
      await app?.close();
    } catch (e) {
      console.error('failed to close electron app', e);
    } finally {
      app = undefined;
      if (userDataDir != null) {
        await rm(userDataDir, { recursive: true, force: true });
        userDataDir = undefined;
      }
    }
  });

  for (const testCase of testCases) {
    test(testCase.name, async () => {
      if (app == null) {
        throw new Error('electron app was not launched');
      }
      const window = await app.firstWindow();
      const driver = createPlaywrightDriver(window, {
        baseURL: 'app://hacker-news.wvb',
        defaultTimeoutMs: 30_000,
      });
      await testCase.run(driver);
    });
  }
});
