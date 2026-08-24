import type { Remote, RemoteConfig } from '@wvb/node';
import { wvbNode } from './native.js';

/** Optional settings forwarded to the native remote client. */
export interface RemoteOptions extends Omit<RemoteConfig, 'baseUrl'> {}

/** Creates a remote-update client for `baseUrl`. */
export function remote(baseUrl: string, options?: RemoteOptions): Remote {
  return new wvbNode.Remote({ baseUrl, ...options });
}
