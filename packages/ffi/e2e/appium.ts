import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { noop } from 'es-toolkit';
import { execa, type ResultPromise } from 'execa';

export const APPIUM_PORT = 4723;

if (!process.env.APPIUM_HOME) {
  process.env.APPIUM_HOME = path.join(os.homedir(), '.appium');
}

/**
 * Driver versions, pinned to the last releases that support the Appium server in
 * `package.json` (3.x). Unpinned installs take the latest driver, which now requires Appium 4.
 */
export const APPIUM_DRIVERS = {
  uiautomator2: '8.7.3',
  xcuitest: '12.22.8',
} as const;

export type AppiumDriver = keyof typeof APPIUM_DRIVERS;

/**
 * Ensures the given Appium drivers are installed into `APPIUM_HOME` (default `~/.appium`) at their
 * pinned versions. A driver at another version (e.g. restored from an older cache) is reinstalled.
 */
export async function ensureAppiumDrivers(drivers: AppiumDriver[]): Promise<void> {
  const installed = await listInstalledDrivers();
  for (const driver of drivers) {
    const version = APPIUM_DRIVERS[driver];
    const current = (installed[driver] as { version?: string } | undefined)?.version;
    if (current === version) {
      continue;
    }
    if (current != null) {
      console.log(`[appium] uninstalling driver: ${driver}@${current}`);
      await execa('appium', ['driver', 'uninstall', driver], {
        preferLocal: true,
        stdout: 'inherit',
        stderr: 'inherit',
      });
    }
    console.log(`[appium] installing driver: ${driver}@${version}`);
    await execa('appium', ['driver', 'install', `${driver}@${version}`], {
      preferLocal: true,
      stdout: 'inherit',
      stderr: 'inherit',
      timeout: 5 * 60_000,
    });
  }
}

async function listInstalledDrivers(): Promise<Record<string, unknown>> {
  const { stdout } = await execa('appium', ['driver', 'list', '--installed', '--json'], {
    preferLocal: true,
    reject: false,
  });
  let installed: Record<string, unknown>;
  try {
    installed = JSON.parse(stdout || '{}');
  } catch {
    installed = {};
  }
  return installed;
}

export interface AppiumServer {
  port: number;
  stop: () => Promise<void>;
}

export async function startAppiumServer(port = APPIUM_PORT): Promise<AppiumServer> {
  console.log(`[appium] starting server on port ${port}`);
  const proc: ResultPromise = execa(
    'appium',
    ['--port', String(port), '--base-path', '/', '--log-level', 'error'],
    { preferLocal: true, stdout: 'inherit', stderr: 'inherit' }
  );
  proc.catch(noop);

  await waitForAppiumServer(port, 60_000);

  return {
    port,
    stop: async () => {
      proc.kill('SIGTERM');
      await proc.catch(noop);
    },
  };
}

async function waitForAppiumServer(port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/status`);
      if (res.ok) {
        return;
      }
    } catch {}
    await delay(500);
  }
  throw new Error(`Appium server did not become ready on port ${port} within ${timeoutMs}ms`);
}
