import type {
  WebviewBundleApi,
  WebviewBundleRemoteApi,
  WebviewBundleSourceApi,
  WebviewBundleUpdaterApi,
} from '../api.js';
import type { IpcInvoke } from '../ipc-spec.js';

const sourceListBundles: IpcInvoke<'webview-bundle:source:list-bundles'> = async () =>
  api().source.listBundles();
const sourceLoadVersion: IpcInvoke<'webview-bundle:source:load-version'> = async bundleName =>
  api().source.loadVersion(bundleName);
const sourceUpdateVersion: IpcInvoke<'webview-bundle:source:update-version'> = async (
  bundleName,
  version
) => api().source.updateVersion(bundleName, version);
const sourceFilepath: IpcInvoke<'webview-bundle:source:resolve-filepath'> = async bundleName =>
  api().source.filepath(bundleName);
const sourceGetBuiltinBundleFilepath: IpcInvoke<
  'webview-bundle:source:get-builtin-bundle-filepath'
> = async (bundleName, version) => api().source.getBuiltinBundleFilepath(bundleName, version);
const sourceGetRemoteBundleFilepath: IpcInvoke<
  'webview-bundle:source:get-remote-bundle-filepath'
> = async (bundleName, version) => api().source.getRemoteBundleFilepath(bundleName, version);
const sourceLoadBuiltinMetadata: IpcInvoke<'webview-bundle:source:load-builtin-metadata'> = async (
  bundleName,
  version
) => api().source.loadBuiltinMetadata(bundleName, version);
const sourceLoadRemoteMetadata: IpcInvoke<'webview-bundle:source:load-remote-metadata'> = async (
  bundleName,
  version
) => api().source.loadRemoteMetadata(bundleName, version);
const sourceUnloadDescriptor: IpcInvoke<
  'webview-bundle:source:unload-descriptor'
> = async bundleName => api().source.unloadDescriptor(bundleName);
const sourceRemoveRemoteBundle: IpcInvoke<'webview-bundle:source:remove-remote-bundle'> = async (
  bundleName,
  version
) => api().source.removeRemoteBundle(bundleName, version);
const sourceRemoteRetainedVersions: IpcInvoke<
  'webview-bundle:source:remote-retained-versions'
> = async bundleName => api().source.remoteRetainedVersions(bundleName);
const sourcePruneRemoteBundles: IpcInvoke<
  'webview-bundle:source:prune-remote-bundles'
> = async bundleName => api().source.pruneRemoteBundles(bundleName);

export const source: WebviewBundleSourceApi = {
  listBundles: sourceListBundles,
  loadVersion: sourceLoadVersion,
  updateVersion: sourceUpdateVersion,
  filepath: sourceFilepath,
  getBuiltinBundleFilepath: sourceGetBuiltinBundleFilepath,
  getRemoteBundleFilepath: sourceGetRemoteBundleFilepath,
  loadBuiltinMetadata: sourceLoadBuiltinMetadata,
  loadRemoteMetadata: sourceLoadRemoteMetadata,
  unloadDescriptor: sourceUnloadDescriptor,
  removeRemoteBundle: sourceRemoveRemoteBundle,
  remoteRetainedVersions: sourceRemoteRetainedVersions,
  pruneRemoteBundles: sourcePruneRemoteBundles,
};

const remoteListBundles: IpcInvoke<'webview-bundle:remote:list-bundles'> = async channel =>
  api().remote.listBundles(channel);
const remoteGetInfo: IpcInvoke<'webview-bundle:remote:get-info'> = async (bundleName, channel) =>
  api().remote.getInfo(bundleName, channel);
const remoteDownload: IpcInvoke<'webview-bundle:remote:download'> = async (bundleName, channel) =>
  api().remote.download(bundleName, channel);
const remoteDownloadVersion: IpcInvoke<'webview-bundle:remote:download-version'> = async (
  bundleName,
  version
) => api().remote.downloadVersion(bundleName, version);

export const remote: WebviewBundleRemoteApi = {
  listBundles: remoteListBundles,
  getInfo: remoteGetInfo,
  download: remoteDownload,
  downloadVersion: remoteDownloadVersion,
};

const updaterListRemotes: IpcInvoke<'webview-bundle:updater:list-remotes'> = async () =>
  api().updater.listRemotes();
const updaterGetUpdate: IpcInvoke<'webview-bundle:updater:get-update'> = async bundleName =>
  api().updater.getUpdate(bundleName);
const updaterDownload: IpcInvoke<'webview-bundle:updater:download'> = async (bundleName, version) =>
  api().updater.download(bundleName, version);
const updaterInstall: IpcInvoke<'webview-bundle:updater:install'> = async (bundleName, version) =>
  api().updater.install(bundleName, version);

export const updater: WebviewBundleUpdaterApi = {
  listRemotes: updaterListRemotes,
  getUpdate: updaterGetUpdate,
  download: updaterDownload,
  install: updaterInstall,
};

function api(): WebviewBundleApi {
  const global = window as any;
  if (global.webviewBundle == null) {
    throw new Error(`Cannot access to webview bundle api.
Make sure to load the preload script before using the api. (via "import { preload } from '@wvb/electron/preload'")`);
  }
  return global.webviewBundle as WebviewBundleApi;
}
