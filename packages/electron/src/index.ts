import './native.js';

export { isWebviewBundleError, WebviewBundleError } from '@wvb/node/binding';
export type {
  BundleProtocolConfig,
  BundleResolverOptions,
  HostnameSegment,
  PathResolver,
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
