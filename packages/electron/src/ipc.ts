import type { Remote, Updater } from '@wvb/node';
import { ipcMain } from 'electron';
import type { WebviewBundle } from './webview-bundle.js';
import { IpcChannels, type IpcHandlerSpecsByScope } from './ipc-spec.js';

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
      wvb.source.updateVersion(bundleName, version),
    [IpcChannels.Source.Filepath]: async (_, bundleName) => wvb.source.filepath(bundleName),
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
    [IpcChannels.Updater.DownloadUpdate]: async (_, remoteName, version) => {
      const info = await updater().downloadUpdate(remoteName, version);
      return info;
    },
  } satisfies IpcHandlerSpecsByScope<'updater'>;

  for (const [channel, handler] of Object.entries(handlers)) {
    ipcMain.handle(channel, handler);
  }
}
