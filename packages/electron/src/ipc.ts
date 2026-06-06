import type { Remote, Updater } from '@wvb/node';
import { ipcMain } from 'electron';
import { IpcChannels, type IpcHandlerSpecsByScope } from './ipc-spec.js';
import type { WebviewBundle } from './webview-bundle.js';

export function registerIpc(wvb: WebviewBundle): void {
  registerSourceIpc(wvb);
  registerRemoteIpc(wvb);
  registerUpdaterIpc(wvb);
}

function registerSourceIpc(wvb: WebviewBundle): void {
  const handlers = {
    [IpcChannels.Source.ListBundles]: async () => wvb.source.listBundles(),
    [IpcChannels.Source.LoadVersion]: async (_, bundleName) => wvb.source.loadVersion(bundleName),
    [IpcChannels.Source.UpdateVersion]: async (_, bundleName, version) =>
      wvb.source.updateRemoteVersion(bundleName, version),
    [IpcChannels.Source.ResolveFilepath]: async (_, bundleName) =>
      wvb.source.resolveFilepath(bundleName),
    [IpcChannels.Source.GetBuiltinBundleFilepath]: async (_, bundleName, version) =>
      wvb.source.getBuiltinBundleFilepath(bundleName, version),
    [IpcChannels.Source.GetRemoteBundleFilepath]: async (_, bundleName, version) =>
      wvb.source.getRemoteBundleFilepath(bundleName, version),
    [IpcChannels.Source.LoadBuiltinMetadata]: async (_, bundleName, version) =>
      wvb.source.loadBuiltinMetadata(bundleName, version),
    [IpcChannels.Source.LoadRemoteMetadata]: async (_, bundleName, version) =>
      wvb.source.loadRemoteMetadata(bundleName, version),
    [IpcChannels.Source.UnloadDescriptor]: async (_, bundleName) =>
      wvb.source.unloadDescriptor(bundleName),
    [IpcChannels.Source.RemoveRemoteBundle]: async (_, bundleName, version) =>
      wvb.source.removeRemoteBundle(bundleName, version),
    [IpcChannels.Source.RemoteRetainedVersions]: async (_, bundleName) =>
      wvb.source.remoteRetainedVersions(bundleName),
    [IpcChannels.Source.PruneRemoteBundles]: async (_, bundleName) =>
      wvb.source.pruneRemoteBundles(bundleName),
  } satisfies IpcHandlerSpecsByScope<'source'>;

  for (const [channel, handler] of Object.entries(handlers)) {
    ipcMain.handle(channel, handler);
  }
}

function registerRemoteIpc(wvb: WebviewBundle): void {
  function remote(): Remote {
    if (wvb.remote == null) {
      throw new Error('remote is not initialized.');
    }
    return wvb.remote;
  }
  const handlers = {
    [IpcChannels.Remote.ListBundles]: async (_, channel) => remote().listBundles(channel),
    [IpcChannels.Remote.GetInfo]: async (_, bundleName, channel) =>
      remote().getInfo(bundleName, channel),
    [IpcChannels.Remote.Download]: async (_, bundleName, channel) => {
      const [info] = await remote().download(bundleName, channel);
      return info;
    },
    [IpcChannels.Remote.DownloadVersion]: async (_, bundleName, version) => {
      const [info] = await remote().downloadVersion(bundleName, version);
      return info;
    },
  } satisfies IpcHandlerSpecsByScope<'remote'>;

  for (const [channel, handler] of Object.entries(handlers)) {
    ipcMain.handle(channel, handler);
  }
}

function registerUpdaterIpc(wvb: WebviewBundle): void {
  function updater(): Updater {
    if (wvb.updater == null) {
      throw new Error('updater is not initialized.');
    }
    return wvb.updater;
  }
  const handlers = {
    [IpcChannels.Updater.ListRemotes]: async () => updater().listRemotes(),
    [IpcChannels.Updater.GetUpdate]: async (_, remoteName) => updater().getUpdate(remoteName),
    [IpcChannels.Updater.Download]: async (_, remoteName, version) => {
      const info = await updater().download(remoteName, version);
      return info;
    },
    [IpcChannels.Updater.Install]: async (_, remoteName, version) => {
      await updater().install(remoteName, version);
    },
  } satisfies IpcHandlerSpecsByScope<'updater'>;

  for (const [channel, handler] of Object.entries(handlers)) {
    ipcMain.handle(channel, handler);
  }
}
