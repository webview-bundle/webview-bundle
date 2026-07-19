/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import type * as binding from '../binding.cjs';
import { isWebviewBundleError, WebviewBundleError } from './error.js';
import { patchBinding, wrapClass, wrapFunction } from './wrap.js';

/**
 * The public `@wvb/node` runtime surface: every export the native `binding.cjs` provides, with the
 * throwing functions/classes wrapped to report {@link WebviewBundleError}, plus the error helpers.
 */
export type WvbNodeBinding = typeof binding & {
  WebviewBundleError: typeof WebviewBundleError;
  isWebviewBundleError: typeof isWebviewBundleError;
};

const BUNDLE_SOURCE_CONFIG_KEYS: ReadonlySet<string> = new Set([
  'builtinDir',
  'remoteDir',
  'builtinManifestFilepath',
  'remoteManifestFilepath',
  'integrity',
  'signature',
  'dataReadOptions',
  'headerReadOptions',
  'indexReadOptions',
]);
const INTEGRITY_KEYS: ReadonlySet<string> = new Set(['policy', 'check', 'checkMode']);
const SIGNATURE_KEYS: ReadonlySet<string> = new Set(['verify', 'verifyMode']);
const READ_OPTIONS_KEYS: ReadonlySet<string> = new Set(['checksum']);
const READ_CHECKSUM_KEYS: ReadonlySet<string> = new Set(['verify', 'seed']);

function assertKnownKeys(what: string, value: object, known: ReadonlySet<string>): void {
  for (const key of Object.keys(value)) {
    if (!known.has(key)) {
      throw new WebviewBundleError('unknown', `wvb: unknown ${what} option '${key}'`);
    }
  }
}

function validateBundleSourceConfig(args: unknown[]): void {
  const [config] = args;
  if (config == null || typeof config !== 'object') {
    return;
  }
  assertKnownKeys('BundleSource', config, BUNDLE_SOURCE_CONFIG_KEYS);
  const { integrity, signature, dataReadOptions, headerReadOptions, indexReadOptions } = config as {
    integrity?: unknown;
    signature?: unknown;
    dataReadOptions?: unknown;
    headerReadOptions?: unknown;
    indexReadOptions?: unknown;
  };
  if (integrity != null && typeof integrity === 'object') {
    assertKnownKeys('BundleSource integrity', integrity, INTEGRITY_KEYS);
  }
  if (signature != null && typeof signature === 'object') {
    assertKnownKeys('BundleSource signature', signature, SIGNATURE_KEYS);
  }
  for (const [name, group] of [
    ['dataReadOptions', dataReadOptions],
    ['headerReadOptions', headerReadOptions],
    ['indexReadOptions', indexReadOptions],
  ] as const) {
    if (group != null && typeof group === 'object') {
      assertKnownKeys(`BundleSource ${name}`, group, READ_OPTIONS_KEYS);
      const { checksum } = group as { checksum?: unknown };
      if (checksum != null && typeof checksum === 'object') {
        assertKnownKeys(`BundleSource ${name}.checksum`, checksum, READ_CHECKSUM_KEYS);
      }
    }
  }
}

/**
 * Build the wrapped `@wvb/node` API from a freshly loaded native `binding.cjs` module. Patches every
 * native class's methods in place, then returns the binding's members with the throwing
 * functions/classes swapped for their {@link WebviewBundleError}-reporting wrappers.
 */
export function buildApi(rawBinding: unknown): WvbNodeBinding {
  const raw = rawBinding as typeof binding;
  patchBinding(raw as unknown as Record<string, unknown>);
  return {
    ...raw,
    readBundle: wrapFunction(raw.readBundle),
    readBundleFromBuffer: wrapFunction(raw.readBundleFromBuffer),
    writeBundle: wrapFunction(raw.writeBundle),
    writeBundleIntoBuffer: wrapFunction(raw.writeBundleIntoBuffer),
    computeIntegrity: wrapFunction(raw.computeIntegrity),
    parseIntegrity: wrapFunction(raw.parseIntegrity),
    BundleSource: wrapClass(raw.BundleSource, validateBundleSourceConfig),
    Remote: wrapClass(raw.Remote),
    Updater: wrapClass(raw.Updater),
    WebviewBundleError,
    isWebviewBundleError,
  };
}
