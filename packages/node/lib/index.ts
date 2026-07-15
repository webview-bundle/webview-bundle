/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import * as binding from '../binding.cjs';
import { WebviewBundleError } from './error.js';
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

const BUNDLE_SOURCE_CONFIG_KEYS: ReadonlySet<string> = new Set([
  'builtinDir',
  'remoteDir',
  'builtinManifestFilepath',
  'remoteManifestFilepath',
  'integrity',
  'signature',
  'verifyDataChecksum',
  'dataChecksumSeed',
]);
const INTEGRITY_KEYS: ReadonlySet<string> = new Set(['policy', 'check', 'checkMode']);
const SIGNATURE_KEYS: ReadonlySet<string> = new Set(['verify', 'verifyMode']);

function assertKnownKeys(what: string, value: object, known: ReadonlySet<string>): void {
  for (const key of Object.keys(value)) {
    if (!known.has(key)) {
      throw new WebviewBundleError('unknown', `wvb: unknown ${what} option '${key}'`);
    }
  }
}

// The native object parser drops unknown keys in silence, so a misspelled security option
// (e.g. `integrity.checkmode`) would leave verification off while the caller believes it is on.
function validateBundleSourceConfig(args: unknown[]): void {
  const [config] = args;
  if (config == null || typeof config !== 'object') {
    return;
  }
  assertKnownKeys('BundleSource', config, BUNDLE_SOURCE_CONFIG_KEYS);
  const { integrity, signature } = config as { integrity?: unknown; signature?: unknown };
  if (integrity != null && typeof integrity === 'object') {
    assertKnownKeys('BundleSource integrity', integrity, INTEGRITY_KEYS);
  }
  if (signature != null && typeof signature === 'object') {
    assertKnownKeys('BundleSource signature', signature, SIGNATURE_KEYS);
  }
}

export type BundleSource = binding.BundleSource;
export const BundleSource: typeof binding.BundleSource = wrapClass(
  binding.BundleSource,
  validateBundleSourceConfig
);

export type Remote = binding.Remote;
export const Remote: typeof binding.Remote = wrapClass(binding.Remote);

export type Updater = binding.Updater;
export const Updater: typeof binding.Updater = wrapClass(binding.Updater);
