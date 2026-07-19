import { remote, source, updater } from '@wvb/bridge';
import type { MethodId } from '../testing/selectors';

/** Runs one bridge method from the string inputs collected by its card. */
export type Invoker = (inputs: Record<string, string>) => Promise<unknown>;

/** Blank optional inputs become `undefined` rather than an empty string. */
function opt(value: string | undefined): string | undefined {
  return value != null && value !== '' ? value : undefined;
}

function req(value: string | undefined): string {
  return value ?? '';
}

/**
 * The implementation of every method in `METHOD_SPECS`: it calls the real
 * `@wvb/bridge` API, which round-trips to the native host.
 */
export const INVOKERS: Record<MethodId, Invoker> = {
  'source.listBundles': () => source.listBundles(),
  'source.loadVersion': i => source.loadVersion(req(i.bundleName)),
  'source.updateVersion': i => source.updateVersion(req(i.bundleName), req(i.version)),
  'source.resolveFilepath': i => source.resolveFilepath(req(i.bundleName)),
  'source.getBuiltinBundleFilepath': i =>
    source.getBuiltinBundleFilepath(req(i.bundleName), req(i.version)),
  'source.getRemoteBundleFilepath': i =>
    source.getRemoteBundleFilepath(req(i.bundleName), req(i.version)),
  'source.loadBuiltinMetadata': i => source.loadBuiltinMetadata(req(i.bundleName), req(i.version)),
  'source.loadRemoteMetadata': i => source.loadRemoteMetadata(req(i.bundleName), req(i.version)),
  'source.unloadDescriptor': i => source.unloadDescriptor(req(i.bundleName)),
  'source.removeRemoteBundle': i => source.removeRemoteBundle(req(i.bundleName), req(i.version)),
  'source.remoteRetainedVersions': i => source.remoteRetainedVersions(req(i.bundleName)),
  'source.pruneRemoteBundles': i => source.pruneRemoteBundles(req(i.bundleName)),
  'remote.listBundles': i => remote.listBundles(opt(i.channel)),
  'remote.getInfo': i => remote.getInfo(req(i.bundleName), opt(i.channel)),
  'remote.download': i => remote.download(req(i.bundleName), opt(i.channel)),
  'remote.downloadVersion': i => remote.downloadVersion(req(i.bundleName), req(i.version)),
  'updater.listRemotes': () => updater.listRemotes(),
  'updater.getUpdate': i => updater.getUpdate(req(i.bundleName)),
  'updater.download': i => updater.download(req(i.bundleName), opt(i.version)),
  'updater.install': i => updater.install(req(i.bundleName), req(i.version)),
};
