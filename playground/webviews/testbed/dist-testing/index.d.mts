import { WebviewDriver, WebviewDriver as WebviewDriver$1 } from "@wvb-playground/testdriver";
//#region testing/methods.d.ts
interface ParamSpec {
  name: string;
  kind: 'string' | 'json';
  defaultValue: string;
}
interface MethodSpec {
  id: string;
  namespace: 'source' | 'remote' | 'updater';
  method: string;
  params: readonly ParamSpec[];
  summary: string;
}
export declare const METHOD_SPECS: readonly [{
  readonly id: 'source.listBundles';
  readonly namespace: 'source';
  readonly method: 'listBundles';
  readonly params: readonly [];
  readonly summary: 'List builtin and remote bundle versions.';
}, {
  readonly id: 'source.listBuiltinBundles';
  readonly namespace: 'source';
  readonly method: 'listBuiltinBundles';
  readonly params: readonly [];
  readonly summary: 'List bundled application assets.';
}, {
  readonly id: 'source.listRemoteBundles';
  readonly namespace: 'source';
  readonly method: 'listRemoteBundles';
  readonly params: readonly [];
  readonly summary: 'List downloaded bundle versions.';
}, {
  readonly id: 'source.getVersion';
  readonly namespace: 'source';
  readonly method: 'getVersion';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }];
  readonly summary: 'Resolve the active bundle version and source.';
}, {
  readonly id: 'source.getRemoteStagedVersion';
  readonly namespace: 'source';
  readonly method: 'getRemoteStagedVersion';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }];
  readonly summary: 'Read the downloaded version waiting for installation.';
}, {
  readonly id: 'source.getRemotePreviousVersion';
  readonly namespace: 'source';
  readonly method: 'getRemotePreviousVersion';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }];
  readonly summary: 'Read the version available for rollback.';
}, {
  readonly id: 'source.getBuiltinVersionData';
  readonly namespace: 'source';
  readonly method: 'getBuiltinVersionData';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }, {
    readonly name: 'version';
    readonly kind: 'string';
    readonly defaultValue: '0.4.0';
  }];
  readonly summary: 'Read builtin integrity and metadata.';
}, {
  readonly id: 'source.getRemoteVersionData';
  readonly namespace: 'source';
  readonly method: 'getRemoteVersionData';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }, {
    readonly name: 'version';
    readonly kind: 'string';
    readonly defaultValue: '0.4.0';
  }];
  readonly summary: 'Read downloaded integrity and metadata.';
}, {
  readonly id: 'source.updateRemoteVersion';
  readonly namespace: 'source';
  readonly method: 'updateRemoteVersion';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }, {
    readonly name: 'version';
    readonly kind: 'string';
    readonly defaultValue: '0.4.0';
  }];
  readonly summary: 'Activate a version recorded in the remote manifest.';
}, {
  readonly id: 'source.updateRemoteVersions';
  readonly namespace: 'source';
  readonly method: 'updateRemoteVersions';
  readonly params: readonly [{
    readonly name: 'items';
    readonly kind: 'json';
    readonly defaultValue: '{}';
  }];
  readonly summary: 'Activate multiple remote versions.';
}, {
  readonly id: 'source.stageRemoteBundle';
  readonly namespace: 'source';
  readonly method: 'stageRemoteBundle';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed-stage';
  }, {
    readonly name: 'data';
    readonly kind: 'json';
    readonly defaultValue: '{"version":"1"}';
  }];
  readonly summary: 'Record a downloaded version as staged.';
}, {
  readonly id: 'source.stageRemoteBundles';
  readonly namespace: 'source';
  readonly method: 'stageRemoteBundles';
  readonly params: readonly [{
    readonly name: 'items';
    readonly kind: 'json';
    readonly defaultValue: '{}';
  }];
  readonly summary: 'Stage multiple versions and their metadata.';
}, {
  readonly id: 'source.removeRemoteBundle';
  readonly namespace: 'source';
  readonly method: 'removeRemoteBundle';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'missing';
  }, {
    readonly name: 'version';
    readonly kind: 'string';
    readonly defaultValue: '0.4.0';
  }, {
    readonly name: 'force';
    readonly kind: 'json';
    readonly defaultValue: 'false';
  }];
  readonly summary: 'Remove a remote version; current versions require force.';
}, {
  readonly id: 'source.removeRemoteBundles';
  readonly namespace: 'source';
  readonly method: 'removeRemoteBundles';
  readonly params: readonly [{
    readonly name: 'items';
    readonly kind: 'json';
    readonly defaultValue: '{}';
  }];
  readonly summary: 'Remove versions from multiple bundles.';
}, {
  readonly id: 'source.pruneRemoteBundle';
  readonly namespace: 'source';
  readonly method: 'pruneRemoteBundle';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'missing';
  }];
  readonly summary: 'Remove orphan versions of a bundle.';
}, {
  readonly id: 'source.pruneRemoteBundles';
  readonly namespace: 'source';
  readonly method: 'pruneRemoteBundles';
  readonly params: readonly [{
    readonly name: 'bundleNames';
    readonly kind: 'json';
    readonly defaultValue: '[]';
  }];
  readonly summary: 'Remove orphan versions of multiple bundles.';
}, {
  readonly id: 'source.resolveFilepath';
  readonly namespace: 'source';
  readonly method: 'resolveFilepath';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }];
  readonly summary: 'Resolve the active bundle file.';
}, {
  readonly id: 'source.getBuiltinBundleFilepath';
  readonly namespace: 'source';
  readonly method: 'getBuiltinBundleFilepath';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }, {
    readonly name: 'version';
    readonly kind: 'string';
    readonly defaultValue: '0.4.0';
  }];
  readonly summary: 'Resolve a builtin bundle file.';
}, {
  readonly id: 'source.getRemoteBundleFilepath';
  readonly namespace: 'source';
  readonly method: 'getRemoteBundleFilepath';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }, {
    readonly name: 'version';
    readonly kind: 'string';
    readonly defaultValue: '0.4.0';
  }];
  readonly summary: 'Resolve a remote bundle file.';
}, {
  readonly id: 'source.unload';
  readonly namespace: 'source';
  readonly method: 'unload';
  readonly params: readonly [{
    readonly name: 'bundleName';
    readonly kind: 'string';
    readonly defaultValue: 'testbed';
  }];
  readonly summary: 'Drop the cached descriptor.';
}, {
  readonly id: 'remote.getUpdate';
  readonly namespace: 'remote';
  readonly method: 'getUpdate';
  readonly params: readonly [{
    readonly name: 'options';
    readonly kind: 'json';
    readonly defaultValue: '';
  }];
  readonly summary: 'Fetch an update document, ETag and signature; unchanged responses are null.';
}, {
  readonly id: 'remote.download';
  readonly namespace: 'remote';
  readonly method: 'download';
  readonly params: readonly [{
    readonly name: 'url';
    readonly kind: 'string';
    readonly defaultValue: 'http://127.0.0.1:1/missing';
  }, {
    readonly name: 'filepath';
    readonly kind: 'string';
    readonly defaultValue: '';
  }];
  readonly summary: 'Download a URL to a file without staging it.';
}, {
  readonly id: 'updater.getUpdate';
  readonly namespace: 'updater';
  readonly method: 'getUpdate';
  readonly params: readonly [{
    readonly name: 'options';
    readonly kind: 'json';
    readonly defaultValue: '';
  }];
  readonly summary: 'Find bundle updates, optionally requiring a signature key ID.';
}, {
  readonly id: 'updater.download';
  readonly namespace: 'updater';
  readonly method: 'download';
  readonly params: readonly [{
    readonly name: 'bundleUpdates';
    readonly kind: 'json';
    readonly defaultValue: '[]';
  }, {
    readonly name: 'options';
    readonly kind: 'json';
    readonly defaultValue: '';
  }];
  readonly summary: 'Download and stage bundle updates; inspect every result.type.';
}, {
  readonly id: 'updater.install';
  readonly namespace: 'updater';
  readonly method: 'install';
  readonly params: readonly [{
    readonly name: 'targets';
    readonly kind: 'json';
    readonly defaultValue: '[]';
  }];
  readonly summary: 'Install staged versions; inspect every result.type.';
}, {
  readonly id: 'updater.rollback';
  readonly namespace: 'updater';
  readonly method: 'rollback';
  readonly params: readonly [{
    readonly name: 'targets';
    readonly kind: 'json';
    readonly defaultValue: '[]';
  }];
  readonly summary: 'Restore previous versions; inspect every result.type.';
}];
type MethodId = (typeof METHOD_SPECS)[number]['id'];
//#endregion
//#region testing/run-method.d.ts
export declare function runMethod(driver: WebviewDriver$1, id: string, params?: Record<string, string>): Promise<{
  status: string | null;
  value: unknown;
}>;
//#endregion
//#region testing/selectors.d.ts
export declare const TESTID: {
  readonly appShell: 'app-shell';
  readonly platformType: 'platform-type';
  readonly version: 'version';
  readonly invokeName: 'invoke-name';
  readonly invokeParams: 'invoke-params';
  readonly invokeRun: 'run-invoke';
  readonly invokeResult: 'result-invoke';
  readonly invokeStatus: 'status-invoke';
};
export declare const byTestId: (id: string) => string;
export declare const tid: {
  method: (id: string) => string;
  param: (id: string, name: string) => string;
  run: (id: string) => string;
  result: (id: string) => string;
  status: (id: string) => string;
};
export declare const sel: Record<keyof typeof TESTID, string>;
export declare const methodSel: {
  method: (id: string) => string;
  param: (id: string, name: string) => string;
  run: (id: string) => string;
  result: (id: string) => string;
  status: (id: string) => string;
};
//#endregion
//#region testing/update-suite.d.ts
interface FixtureRemote {
  endpoint: string;
  setVersion(version: string): void;
}
export declare function defineTestbedUpdateSuite(getDriver: () => WebviewDriver$1, getRemote: () => FixtureRemote): void;
//#endregion
//#region testing/index.d.ts
export declare const testCases: Array<{
  name: string;
  run: (driver: WebviewDriver$1) => Promise<void>;
}>;
export declare function defineTestbedSuite(getDriver: () => WebviewDriver$1): void;
//#endregion
export type { MethodId, MethodSpec, WebviewDriver };