import { Option } from 'clipanion';
import { z } from 'zod';

export const AppleTargetSchema = z.enum([
  'aarch64-apple-ios',
  'aarch64-apple-darwin',
  'aarch64-apple-ios-sim',
  'x86_64-apple-ios',
  'x86_64-apple-darwin',
]);
export type AppleTarget = z.infer<typeof AppleTargetSchema>;

export const AppleTargetOption = Option.Array('--target', {
  description: 'Set the target platform for Apple compilation. [Default: all]',
});

export type ApplePlatform = 'macos' | 'ios' | 'ios-simulator';

export function getApplePlatformFromTarget(target: AppleTarget): ApplePlatform {
  switch (target) {
    case 'aarch64-apple-darwin':
    case 'x86_64-apple-darwin':
      return 'macos';
    case 'aarch64-apple-ios':
      return 'ios';
    case 'aarch64-apple-ios-sim':
    case 'x86_64-apple-ios':
      // x86_64-apple-ios is the iOS simulator target for Intel Macs
      return 'ios-simulator';
  }
}

export const AndroidTargetSchema = z.enum([
  'x86_64-linux-android',
  'aarch64-linux-android',
  'armv7-linux-androideabi',
  'i686-linux-android',
]);
export type AndroidTarget = z.infer<typeof AndroidTargetSchema>;

export const AndroidTargetOption = Option.Array('--target', {
  description: 'Set the target for Android compilation. [Default: all]',
});
