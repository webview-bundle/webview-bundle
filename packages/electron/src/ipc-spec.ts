import type { IpcMainInvokeEvent } from 'electron';
import type {
  BundleManifestMetadata,
  BundleSourceVersion,
  BundleUpdateInfo,
  ListBundleItem,
  ListRemoteBundleInfo,
  RemoteBundleInfo,
} from './api.js';

export const IpcChannels = {
  Source: {
    ListBundles: 'webview-bundle:source:list-bundles',
    LoadVersion: 'webview-bundle:source:load-version',
    UpdateVersion: 'webview-bundle:source:update-version',
    ResolveFilepath: 'webview-bundle:source:resolve-filepath',
    GetBuiltinBundleFilepath: 'webview-bundle:source:get-builtin-bundle-filepath',
    GetRemoteBundleFilepath: 'webview-bundle:source:get-remote-bundle-filepath',
    LoadBuiltinMetadata: 'webview-bundle:source:load-builtin-metadata',
    LoadRemoteMetadata: 'webview-bundle:source:load-remote-metadata',
    UnloadDescriptor: 'webview-bundle:source:unload-descriptor',
    RemoveRemoteBundle: 'webview-bundle:source:remove-remote-bundle',
    RemoteRetainedVersions: 'webview-bundle:source:remote-retained-versions',
    PruneRemoteBundles: 'webview-bundle:source:prune-remote-bundles',
  },
  Remote: {
    ListBundles: 'webview-bundle:remote:list-bundles',
    GetInfo: 'webview-bundle:remote:get-info',
    Download: 'webview-bundle:remote:download',
    DownloadVersion: 'webview-bundle:remote:download-version',
  },
  Updater: {
    ListRemotes: 'webview-bundle:updater:list-remotes',
    GetUpdate: 'webview-bundle:updater:get-update',
    Download: 'webview-bundle:updater:download',
    Install: 'webview-bundle:updater:install',
  },
} as const;

type ValueOf<T> = T[keyof T];
type DeepValueOf<T> = T extends object ? ValueOf<{ [K in keyof T]: DeepValueOf<T[K]> }> : T;

export type IpcChannelScope = Lowercase<keyof typeof IpcChannels>;
export type IpcChannel = DeepValueOf<typeof IpcChannels>;

export type IpcHandler<Return = unknown, Args extends unknown[] = []> = (
  event: IpcMainInvokeEvent,
  ...args: Args
) => Promise<Return>;
export type IpcHandlerSpecs = {
  // source
  'webview-bundle:source:list-bundles': IpcHandler<ListBundleItem[]>;
  'webview-bundle:source:load-version': IpcHandler<
    BundleSourceVersion | null,
    [bundleName: string]
  >;
  'webview-bundle:source:update-version': IpcHandler<void, [bundleName: string, version: string]>;
  'webview-bundle:source:resolve-filepath': IpcHandler<string, [bundleName: string]>;
  'webview-bundle:source:get-builtin-bundle-filepath': IpcHandler<
    string,
    [bundleName: string, version: string]
  >;
  'webview-bundle:source:get-remote-bundle-filepath': IpcHandler<
    string,
    [bundleName: string, version: string]
  >;
  'webview-bundle:source:load-builtin-metadata': IpcHandler<
    BundleManifestMetadata | null,
    [bundleName: string, version: string]
  >;
  'webview-bundle:source:load-remote-metadata': IpcHandler<
    BundleManifestMetadata | null,
    [bundleName: string, version: string]
  >;
  'webview-bundle:source:unload-descriptor': IpcHandler<boolean, [bundleName: string]>;
  'webview-bundle:source:remove-remote-bundle': IpcHandler<
    boolean,
    [bundleName: string, version: string]
  >;
  'webview-bundle:source:remote-retained-versions': IpcHandler<string[], [bundleName: string]>;
  'webview-bundle:source:prune-remote-bundles': IpcHandler<string[], [bundleName: string]>;
  // remote
  'webview-bundle:remote:list-bundles': IpcHandler<
    ListRemoteBundleInfo[],
    [channel?: string | undefined]
  >;
  'webview-bundle:remote:get-info': IpcHandler<
    RemoteBundleInfo,
    [bundleName: string, channel?: string | undefined]
  >;
  'webview-bundle:remote:download': IpcHandler<
    RemoteBundleInfo,
    [bundleName: string, channel?: string | undefined]
  >;
  'webview-bundle:remote:download-version': IpcHandler<
    RemoteBundleInfo,
    [bundleName: string, version: string]
  >;
  // updater
  'webview-bundle:updater:list-remotes': IpcHandler<ListRemoteBundleInfo[]>;
  'webview-bundle:updater:get-update': IpcHandler<BundleUpdateInfo, [bundleName: string]>;
  'webview-bundle:updater:download': IpcHandler<
    RemoteBundleInfo,
    [bundleName: string, version?: string | undefined]
  >;
  'webview-bundle:updater:install': IpcHandler<void, [bundleName: string, version: string]>;
};
export type IpcHandlerSpecsByScope<Scope extends IpcChannelScope> = {
  [K in Extract<keyof IpcHandlerSpecs, `webview-bundle:${Scope}:${string}`>]: IpcHandlerSpecs[K];
};

type IpcHandlerArgs<T extends IpcChannel> = IpcHandlerSpecs[T] extends (
  event: IpcMainInvokeEvent,
  ...args: infer Args
) => any
  ? Args
  : never;
type IpcHandlerReturn<T extends IpcChannel> = IpcHandlerSpecs[T] extends (
  ...args: any[]
) => Promise<infer Return>
  ? Return
  : never;

export type IpcInvoke<T extends IpcChannel> = (
  ...args: IpcHandlerArgs<T>
) => Promise<IpcHandlerReturn<T>>;
