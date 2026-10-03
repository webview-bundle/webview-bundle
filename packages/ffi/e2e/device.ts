import type { DeviceProvider } from '@e2e-dev/mobile';
import { createAgentDeviceClient } from 'agent-device';

// Select once during engine preparation, so booting cannot change the config.
export const device: DeviceProvider = {
  name: 'local-device',
  async acquire({ platform, slot, log }) {
    const allDevices = await createAgentDeviceClient().devices.list({ platform });
    const devices = allDevices
      .filter(x => x.target === 'mobile' && x.kind !== 'device')
      .sort((a, b) => Number(b.booted) - Number(a.booted));
    const device = devices[slot];

    if (device == null) {
      throw new Error(`No ${platform} simulator/emulator available for worker ${slot}`);
    }

    log(`selected ${device.name} (${device.id})`);

    return {
      id: device.id,
      // A stopped AVD gets a new adb serial when booted; its name stays usable.
      device: platform === 'android' && !device.booted ? device.name : device.id,
      client: {}, // Use the local agent-device daemon.
    };
  },
  async release() {},
};
