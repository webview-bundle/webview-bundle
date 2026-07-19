/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import * as binding from '../binding.cjs';
import { buildApi } from './api.js';

const api = buildApi(binding);

export * from '../binding.cjs';
export { isWebviewBundleError, WebviewBundleError } from './error.js';

export const readBundle: typeof binding.readBundle = api.readBundle;
export const readBundleFromBuffer: typeof binding.readBundleFromBuffer = api.readBundleFromBuffer;
export const writeBundle: typeof binding.writeBundle = api.writeBundle;
export const writeBundleIntoBuffer: typeof binding.writeBundleIntoBuffer =
  api.writeBundleIntoBuffer;
export const computeIntegrity: typeof binding.computeIntegrity = api.computeIntegrity;
export const parseIntegrity: typeof binding.parseIntegrity = api.parseIntegrity;

export type BundleSource = binding.BundleSource;
export const BundleSource: typeof binding.BundleSource = api.BundleSource;

export type Remote = binding.Remote;
export const Remote: typeof binding.Remote = api.Remote;

export type Updater = binding.Updater;
export const Updater: typeof binding.Updater = api.Updater;
