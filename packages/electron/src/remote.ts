import type { Remote, RemoteOptions as RemoteBindingOptions } from '@wvb/node';
import { wvbNode } from './native.js';

export interface RemoteOptions extends RemoteBindingOptions {}

export function remote(endpoint: string, options?: RemoteOptions): Remote {
  return new wvbNode.Remote(endpoint, options);
}
