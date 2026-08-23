import type { Remote, RemoteConfig } from '@wvb/node';
import { wvbNode } from './native.js';

export interface RemoteOptions extends Omit<RemoteConfig, 'baseUrl'> {}

export function remote(baseUrl: string, options?: RemoteOptions): Remote {
  return new wvbNode.Remote({ baseUrl, ...options });
}
