// remote — construct a @wvb/deno Remote (mirrors @wvb/electron's remote.ts).
import { Remote, type RemoteOptions as RemoteBindingOptions } from '@wvb/deno';

export interface RemoteOptions extends RemoteBindingOptions {}

export function remote(endpoint: string, options?: RemoteOptions): Remote {
  return new Remote(endpoint, options);
}
