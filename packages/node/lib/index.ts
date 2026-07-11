/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import * as binding from '../binding.cjs';
import { patchBinding, wrapClass, wrapFunction } from './wrap.js';

patchBinding(binding as unknown as Record<string, unknown>);

export * from '../binding.cjs';
export { isWebviewBundleError, WebviewBundleError } from './error.js';

export const readBundle: typeof binding.readBundle = wrapFunction(binding.readBundle);
export const readBundleFromBuffer: typeof binding.readBundleFromBuffer = wrapFunction(
  binding.readBundleFromBuffer
);
export const writeBundle: typeof binding.writeBundle = wrapFunction(binding.writeBundle);
export const writeBundleIntoBuffer: typeof binding.writeBundleIntoBuffer = wrapFunction(
  binding.writeBundleIntoBuffer
);

export type Remote = binding.Remote;
export const Remote: typeof binding.Remote = wrapClass(binding.Remote);

export type Updater = binding.Updater;
export const Updater: typeof binding.Updater = wrapClass(binding.Updater);
