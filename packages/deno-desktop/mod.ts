// @wvb/deno-desktop — serve webview bundles in a Deno desktop app. API mirrors @wvb/electron.

// Deno-specific: re-export the native library loaders so apps don't need a separate @wvb/deno import.
export { type HttpResponse, loadLib, loadLibViaPlug } from '@wvb/deno';
export type {
  BundleProtocolConfig,
  LocalProtocolConfig,
  Protocol,
  ProtocolHandler,
  ProtocolHandlerBuild,
  ProtocolHandlerBuildContext,
  ProtocolOptions,
} from './lib/protocol.ts';
export { bundleProtocol, localProtocol } from './lib/protocol.ts';
export type { RemoteOptions } from './lib/remote.ts';
export { remote } from './lib/remote.ts';
export type { SourceOptions } from './lib/source.ts';
export { appDataDir, bundleSource } from './lib/source.ts';
export type {
  WebviewBundle,
  WebviewBundleConfig,
  WebviewBundleRemoteConfig,
  WebviewBundleUpdaterConfig,
} from './lib/webview-bundle.ts';
export { webviewBundle, wvb } from './lib/webview-bundle.ts';
