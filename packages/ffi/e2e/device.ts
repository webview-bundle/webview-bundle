import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { execa } from 'execa';

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
    { detached: true, stdio: 'ignore' }
  );
  proc.unref();

  try {
    // Bound the wait so a wedged emulator surfaces a clear error instead of hanging the caller's
    // hook until its (long) timeout fires.
    await execa('adb', ['wait-for-device'], { timeout: 180_000 });
    await waitForAndroidBoot(180_000);

    const udid = (await listAndroidDevices())[0];
    if (!udid) {
      throw new Error('Android emulator booted but no device is attached.');
    }
    return {
      udid,
      bootedByUs: true,
      shutdown: async () => {
        await execa('adb', ['-s', udid, 'emu', 'kill'], { reject: false });
      },
    };
  } catch (err) {
    // Boot failed/timed out before we could hand back a handle — don't leak the emulator we spawned.
    proc.kill('SIGKILL');
    throw err;
  }
}

async function waitForAndroidBoot(timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const { stdout } = await execa('adb', ['shell', 'getprop', 'sys.boot_completed'], {
      reject: false,
    });
    if (stdout.trim() === '1') {
      return;
    }
    await delay(2000);
  }
  throw new Error('Android emulator did not finish booting in time.');
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
