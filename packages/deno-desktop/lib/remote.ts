// remote — construct a @wvb/deno Remote (mirrors @wvb/electron's remote.ts).
import { Remote, type RemoteConfig } from '@wvb/deno';

export type { RemoteConfig };

export function remote(config: RemoteConfig): Remote {
  return new Remote(config);
}
