import type { IpcMainInvokeEvent } from 'electron';
import type {
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
    Filepath: 'webview-bundle:source:filepath',
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
    DownloadUpdate: 'webview-bundle:updater:download-update',
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
  'webview-bundle:source:filepath': IpcHandler<string, [bundleName: string]>;
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
  'webview-bundle:updater:download-update': IpcHandler<
    RemoteBundleInfo,
    [bundleName: string, version?: string | undefined]
  >;
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
