// @wvb/deno — webview-bundle core API bound to the Deno runtime via Deno FFI.
//
// The Deno peer of `@wvb/node` (napi): it re-exports the core API as Deno-native classes, backed by
// the `wvb-deno` cdylib through `Deno.dlopen`. Load the native library first (`loadLib` /
// `loadLibViaPlug`), then use the bindings. For Deno desktop apps, see `@wvb/deno-desktop`.
export { type HttpResponse, loadLib, loadLibViaPlug, platformLibFileName } from './lib/ffi.ts';
export { BundleProtocol, type HttpMethod, LocalProtocol, toResponse } from './lib/protocol.ts';
export {
  type HttpOptions,
  type ListRemoteBundleInfo,
  Remote,
  type RemoteBundleInfo,
  type RemoteDownload,
  type RemoteOptions,
} from './lib/remote.ts';
export { BundleSource, type BundleSourceConfig } from './lib/source.ts';
export {
  type BundleUpdateInfo,
  type IntegrityPolicy,
  Updater,
  type UpdaterOptions,
} from './lib/updater.ts';
