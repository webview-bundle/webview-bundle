import path from 'node:path';
import { mobile } from '@e2e-dev/mobile';
import type { E2EConfig } from 'e2e';
import { device } from './e2e/device.js';

export default {
  tests: ['e2e/**/*.e2e.ts'],
  targets: [
    {
      name: 'android',
      engine: mobile({ platform: 'android', device: process.env.ANDROID_AVD ?? device }),
      app: {
        bundleId: 'dev.wvb.testapp',
        appPath: path.join(
          import.meta.dirname,
          'android/testapp/build/outputs/apk/debug/testapp-debug.apk'
        ),
      },
    },
    {
      name: 'apple',
      engine: mobile({ platform: 'ios', device: process.env.IOS_DEVICE ?? device }),
      app: {
        bundleId: 'dev.wvb.testapp',
        appPath: path.resolve(
          import.meta.dirname,
          '.output/ios-simulator/Build/Products/Debug-iphonesimulator/TestApp.app'
        ),
      },
    },
  ],
  workers: 1,
  retries: 0,
  timeout: 360_000,
  launchTimeout: 720_000,
  actionTimeout: 120_000,
  assertionTimeout: 30_000,
  reporters: ['list', 'junit'],
} satisfies E2EConfig;
