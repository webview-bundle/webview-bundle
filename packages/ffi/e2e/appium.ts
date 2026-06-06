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
 * Ensures the given Appium drivers are installed into `APPIUM_HOME` (default `~/.appium`).
 */
export async function ensureAppiumDrivers(drivers: string[]): Promise<void> {
  const installed = await listInstalledDrivers();
  for (const driver of drivers) {
    if (driver in installed) {
      continue;
    }
    console.log(`[appium] installing driver: ${driver}`);
    await execa('appium', ['driver', 'install', driver], {
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
