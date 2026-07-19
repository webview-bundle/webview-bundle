/**
 * The `data-testid` contract the bridge testbed UI must satisfy. These ids are
 * the single source of truth shared between the app's markup (`src/`) and the
 * E2E cases ({@link ./cases}). The app builds its markup from {@link METHOD_SPECS}
 * and the builders below, so keeping a method here in sync automatically keeps
 * the UI and the tests in sync.
 */

/** Static ids for the fixed chrome of the testbed (not per-method). */
export const TESTID = {
  /** App shell; also carries `data-platform`. */
  appShell: 'app-shell',
  /** Detected platform type, or `none`. */
  platformType: 'platform-type',
  /** Raw `invoke()` escape hatch. */
  invokeName: 'invoke-name',
  invokeParams: 'invoke-params',
  invokeRun: 'run-invoke',
  invokeResult: 'result-invoke',
  invokeStatus: 'status-invoke',
} as const;

/** The bridge namespaces exercised by the testbed. */
export type Namespace = 'source' | 'remote' | 'updater';

/** A single input of a bridge method. */
export interface MethodParam {
  /** Matches the parameter name the bridge method expects. */
  name: string;
  /** Whether the parameter is optional (rendered but may be left blank). */
  optional?: boolean;
}

/** How the method's result is rendered, so tests know what to expect. */
export type ResultKind = 'value' | 'void';

/** Declarative description of one bridge method under test. */
export interface MethodSpec {
  /** Stable id, `"<namespace>.<method>"`; drives every per-method testid. */
  id: string;
  namespace: Namespace;
  method: string;
  params: readonly MethodParam[];
  resultKind: ResultKind;
  /** One-line description shown on the method card. */
  summary: string;
}

/**
 * Every bridge method the testbed drives, grouped by namespace and in the same
 * order as the `@wvb/bridge` API surface. Add a method here and it appears in the
 * UI and (via {@link ./cases}) the generated test cases.
 */
export const METHOD_SPECS = [
  {
    id: 'source.listBundles',
    namespace: 'source',
    method: 'listBundles',
    params: [],
    resultKind: 'value',
    summary: 'List every builtin and remote bundle known to the source.',
  },
  {
    id: 'source.loadVersion',
    namespace: 'source',
    method: 'loadVersion',
    params: [{ name: 'bundleName' }],
    resultKind: 'value',
    summary: 'Resolve the currently active version of a bundle.',
  },
  {
    id: 'source.updateVersion',
    namespace: 'source',
    method: 'updateVersion',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'void',
    summary: 'Point a bundle at a specific installed version.',
  },
  {
    id: 'source.resolveFilepath',
    namespace: 'source',
    method: 'resolveFilepath',
    params: [{ name: 'bundleName' }],
    resultKind: 'value',
    summary: 'Resolve the on-disk path of the active bundle file.',
  },
  {
    id: 'source.getBuiltinBundleFilepath',
    namespace: 'source',
    method: 'getBuiltinBundleFilepath',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'value',
    summary: 'Path of a specific builtin bundle version.',
  },
  {
    id: 'source.getRemoteBundleFilepath',
    namespace: 'source',
    method: 'getRemoteBundleFilepath',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'value',
    summary: 'Path of a specific downloaded remote bundle version.',
  },
  {
    id: 'source.loadBuiltinMetadata',
    namespace: 'source',
    method: 'loadBuiltinMetadata',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'value',
    summary: 'Manifest metadata for a builtin bundle version.',
  },
  {
    id: 'source.loadRemoteMetadata',
    namespace: 'source',
    method: 'loadRemoteMetadata',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'value',
    summary: 'Manifest metadata for a remote bundle version.',
  },
  {
    id: 'source.unloadDescriptor',
    namespace: 'source',
    method: 'unloadDescriptor',
    params: [{ name: 'bundleName' }],
    resultKind: 'value',
    summary: 'Drop the in-memory descriptor for a bundle.',
  },
  {
    id: 'source.removeRemoteBundle',
    namespace: 'source',
    method: 'removeRemoteBundle',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'value',
    summary: 'Delete a downloaded remote bundle version from disk.',
  },
  {
    id: 'source.remoteRetainedVersions',
    namespace: 'source',
    method: 'remoteRetainedVersions',
    params: [{ name: 'bundleName' }],
    resultKind: 'value',
    summary: 'Versions currently retained for a remote bundle.',
  },
  {
    id: 'source.pruneRemoteBundles',
    namespace: 'source',
    method: 'pruneRemoteBundles',
    params: [{ name: 'bundleName' }],
    resultKind: 'value',
    summary: 'Prune non-retained remote bundle versions.',
  },
  {
    id: 'remote.listBundles',
    namespace: 'remote',
    method: 'listBundles',
    params: [{ name: 'channel', optional: true }],
    resultKind: 'value',
    summary: 'List bundles available on the remote (optionally by channel).',
  },
  {
    id: 'remote.getInfo',
    namespace: 'remote',
    method: 'getInfo',
    params: [{ name: 'bundleName' }, { name: 'channel', optional: true }],
    resultKind: 'value',
    summary: 'Fetch remote info (version, etag, integrity, signature).',
  },
  {
    id: 'remote.download',
    namespace: 'remote',
    method: 'download',
    params: [{ name: 'bundleName' }, { name: 'channel', optional: true }],
    resultKind: 'value',
    summary: 'Download the latest remote bundle for a channel.',
  },
  {
    id: 'remote.downloadVersion',
    namespace: 'remote',
    method: 'downloadVersion',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'value',
    summary: 'Download a specific remote bundle version.',
  },
  {
    id: 'updater.listRemotes',
    namespace: 'updater',
    method: 'listRemotes',
    params: [],
    resultKind: 'value',
    summary: 'List remotes the updater is configured to check.',
  },
  {
    id: 'updater.getUpdate',
    namespace: 'updater',
    method: 'getUpdate',
    params: [{ name: 'bundleName' }],
    resultKind: 'value',
    summary: 'Check whether a newer bundle version is available.',
  },
  {
    id: 'updater.download',
    namespace: 'updater',
    method: 'download',
    params: [{ name: 'bundleName' }, { name: 'version', optional: true }],
    resultKind: 'value',
    summary: 'Download an update (latest, or a specific version).',
  },
  {
    id: 'updater.install',
    namespace: 'updater',
    method: 'install',
    params: [{ name: 'bundleName' }, { name: 'version' }],
    resultKind: 'void',
    summary: 'Install a downloaded update as the active version.',
  },
] as const satisfies readonly MethodSpec[];

/** Union of every method id in {@link METHOD_SPECS}. */
export type MethodId = (typeof METHOD_SPECS)[number]['id'];

/** Build a `data-testid` attribute selector. */
export function byTestId(id: string): string {
  return `[data-testid="${id}"]`;
}

/**
 * Per-method `data-testid` string builders (kept in sync with the app markup).
 * `result` is the single terminal-output element for a method; it carries
 * `data-status="ok" | "error"` and renders either the value or the error.
 */
export const tid = {
  method: (id: string): string => `method-${id}`,
  param: (id: string, name: string): string => `param-${id}-${name}`,
  run: (id: string): string => `run-${id}`,
  result: (id: string): string => `result-${id}`,
  status: (id: string): string => `status-${id}`,
} as const;

/** Ready-made CSS selectors for the fixed chrome. */
export const sel = {
  appShell: byTestId(TESTID.appShell),
  platformType: byTestId(TESTID.platformType),
  invokeName: byTestId(TESTID.invokeName),
  invokeParams: byTestId(TESTID.invokeParams),
  invokeRun: byTestId(TESTID.invokeRun),
  invokeResult: byTestId(TESTID.invokeResult),
  invokeStatus: byTestId(TESTID.invokeStatus),
} as const;

/** Per-method CSS selector builders. */
export const methodSel = {
  method: (id: string): string => byTestId(tid.method(id)),
  param: (id: string, name: string): string => byTestId(tid.param(id, name)),
  run: (id: string): string => byTestId(tid.run(id)),
  result: (id: string): string => byTestId(tid.result(id)),
  status: (id: string): string => byTestId(tid.status(id)),
} as const;
