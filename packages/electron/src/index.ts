import './native.js';

export type {
  BundleResolverOptions,
  HostnameSegment,
  PathResolver,
} from '@wvb/node';
// Re-export the commonly-used @wvb/node runtime helpers so consumers do not have to
// import '@wvb/node' directly. Importing @wvb/node before @wvb/electron would load its
// native binding before `./native.js` can point it at the bundled binary, making the
// override a no-op.
export { isWebviewBundleError, WebviewBundleError } from '@wvb/node';
export type {
  BundleProtocolConfig,
  Protocol,
  ProtocolHandler,
  ProtocolOptions,
  ProxyProtocolConfig,
  ProxyResolver,
} from './protocol.js';
export { bundleProtocol, proxyProtocol } from './protocol.js';
export type { RemoteOptions } from './remote.js';
export type { SourceOptions } from './source.js';
export type {
  WebviewBundle,
  WebviewBundleConfig,
  WebviewBundleRemoteConfig,
} from './webview-bundle.js';
export { webviewBundle, wvb } from './webview-bundle.js';
