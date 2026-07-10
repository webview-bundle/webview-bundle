import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { execa, type ResultPromise } from 'execa';

const BOOT_TIMEOUT_MS = 180_000;

export interface AndroidDevice {
  udid: string;
  /** Whether this process booted the device (callers decide whether to shut it down). */
  bootedByUs: boolean;
  shutdown: () => Promise<void>;
}

async function listAndroidDevices(): Promise<string[]> {
  const { stdout } = await execa('adb', ['devices'], { reject: false });
  return stdout
    .split('\n')
    .slice(1)
    .map(line => line.trim())
    .filter(line => line.endsWith('\tdevice'))
    .map(line => line.split('\t')[0]!)
    .filter(Boolean);
}

async function listAvds(emulatorBin: string): Promise<string[]> {
  const { stdout } = await execa(emulatorBin, ['-list-avds'], { reject: false });
  // Some emulator builds interleave INFO/WARNING banners with the names; AVD names themselves are
  // restricted to `[A-Za-z0-9._-]`, so anything else is a banner.
  return stdout
    .split('\n')
    .map(line => line.trim())
    .filter(line => /^[A-Za-z0-9._-]+$/.test(line));
}

export async function ensureAndroidDevice(avd: string): Promise<AndroidDevice> {
  const existing = await listAndroidDevices();
  if (existing.length > 0) {
    return { udid: existing[0]!, bootedByUs: false, shutdown: async () => {} };
  }

  const sdk = process.env.ANDROID_HOME ?? process.env.ANDROID_SDK_ROOT;
  if (sdk == null) {
    throw new Error('ANDROID_HOME / ANDROID_SDK_ROOT is not set; cannot launch an emulator.');
  }
  const emulatorBin = path.join(sdk, 'emulator', 'emulator');

  // Check up front: a bad AVD name makes the emulator die instantly, which would otherwise only
  // surface 180s later as an `adb wait-for-device` timeout pointing at the wrong step.
  const avds = await listAvds(emulatorBin);
  if (!avds.includes(avd)) {
    const available = avds.length > 0 ? avds.join(', ') : '(none)';
    throw new Error(
      `Android AVD "${avd}" does not exist. Available AVDs: ${available}. ` +
        'Set ANDROID_AVD to one of them, or create it with `avdmanager create avd`.'
    );
  }

  console.log(`[device] booting Android emulator: ${avd}`);
  const proc = execa(
    emulatorBin,
    [
      '-avd',
      avd,
      '-no-window',
      '-no-audio',
      '-no-boot-anim',
      '-no-snapshot',
      '-gpu',
      'swiftshader_indirect',
    ],
    // stderr is piped (not ignored) so an early exit can report *why* the emulator refused to boot.
    // `reject: false` keeps that exit from becoming an unhandled rejection.
    { detached: true, stdin: 'ignore', stdout: 'ignore', stderr: 'pipe', reject: false }
  );
  proc.unref();

  // Aborts the boot polling if `watchEmulatorExit` wins the race below, so we don't leave an `adb`
  // child running for the rest of the timeout.
  const controller = new AbortController();
  try {
    // Bound the wait so a wedged emulator surfaces a clear error instead of hanging the caller's
    // hook until its (long) timeout fires. Racing against the emulator process means a crash is
    // reported immediately rather than as a timeout.
    await Promise.race([watchEmulatorExit(proc, avd), waitForBoot(controller.signal)]);

    const udid = (await listAndroidDevices())[0];
    if (!udid) {
      throw new Error('Android emulator booted but no device is attached.');
    }
    // `subprocess.unref()` detaches only the process handle — the piped stderr socket would still
    // hold the event loop open, hanging vitest whenever WVB_E2E_KEEP leaves the emulator running.
    (proc.stderr as { unref?: () => void } | null)?.unref?.();
    return {
      udid,
      bootedByUs: true,
      shutdown: async () => {
        await execa('adb', ['-s', udid, 'emu', 'kill'], { reject: false });
      },
    };
  } catch (err) {
    // Boot failed/timed out before we could hand back a handle — don't leak the emulator we spawned.
    controller.abort();
    killProcessGroup(proc);
    throw err;
  }
}

/** Resolves never: rejects as soon as the emulator exits, whatever its exit code. */
async function watchEmulatorExit(proc: ResultPromise, avd: string): Promise<never> {
  const result = await proc;
  const stderr = typeof result.stderr === 'string' ? result.stderr.trim() : '';
  const code = result.exitCode != null ? ` (exit code ${result.exitCode})` : '';
  throw new Error(
    `Android emulator for "${avd}" exited before finishing boot${code}.` +
      (stderr ? `\n${stderr}` : '')
  );
}

async function waitForBoot(signal: AbortSignal): Promise<void> {
  await execa('adb', ['wait-for-device'], { timeout: BOOT_TIMEOUT_MS, cancelSignal: signal });
  const deadline = Date.now() + BOOT_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const { stdout } = await execa('adb', ['shell', 'getprop', 'sys.boot_completed'], {
      reject: false,
      cancelSignal: signal,
    });
    if (stdout.trim() === '1') {
      return;
    }
    await delay(2000, undefined, { signal });
  }
  throw new Error('Android emulator did not finish booting in time.');
}

function killProcessGroup(proc: ResultPromise): void {
  const { pid } = proc;
  if (pid == null) {
    return;
  }
  try {
    // `detached: true` makes the emulator its own process-group leader (pgid === pid). Signalling
    // the negative pid also takes down the helpers it spawns (netsimd, crash handler), which a
    // plain `proc.kill()` would orphan.
    process.kill(-pid, 'SIGKILL');
  } catch {
    // Already reaped, or no such group — fall back to the direct kill.
    proc.kill('SIGKILL');
  }
}

export interface IosSimulator extends AndroidDevice {
  name: string;
}

interface SimDevice {
  udid: string;
  name: string;
  state: string;
}

async function listIosSimDevices(): Promise<SimDevice[]> {
  const { stdout } = await execa('xcrun', ['simctl', 'list', 'devices', 'available', '--json']);
  const parsed = JSON.parse(stdout) as {
    devices: Record<string, Array<{ udid: string; name: string; state: string }>>;
  };
  const out: SimDevice[] = [];
  for (const [runtime, devices] of Object.entries(parsed.devices)) {
    if (!/iOS/i.test(runtime)) {
      continue;
    }
    for (const d of devices) {
      out.push({ udid: d.udid, name: d.name, state: d.state });
    }
  }
  return out;
}

export async function ensureIosSimulator(deviceName: string): Promise<IosSimulator> {
  const devices = await listIosSimDevices();

  const booted = devices.find(d => d.state === 'Booted');
  if (booted != null) {
    return { udid: booted.udid, name: booted.name, bootedByUs: false, shutdown: async () => {} };
  }

  // Prefer the requested device, but fall back to any available iPhone so this works across CI
  // runners whose Xcode ships a different simulator lineup.
  const target =
    devices.find(d => d.name === deviceName) ?? devices.find(d => /^iPhone/i.test(d.name));
  if (target == null) {
    throw new Error(`No iOS simulator found (looked for "${deviceName}" or any iPhone).`);
  }
  if (target.name !== deviceName) {
    console.log(`[device] "${deviceName}" not available; falling back to ${target.name}`);
  }
  console.log(`[device] booting iOS simulator: ${target.name} (${target.udid})`);
  await execa('xcrun', ['simctl', 'boot', target.udid]);
  await execa('xcrun', ['simctl', 'bootstatus', target.udid]); // blocks until fully booted
  return {
    udid: target.udid,
    name: target.name,
    bootedByUs: true,
    shutdown: async () => {
      await execa('xcrun', ['simctl', 'shutdown', target.udid], { reject: false });
    },
  };
}
