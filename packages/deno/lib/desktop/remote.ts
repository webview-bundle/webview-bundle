// remote — construct a @wvb/deno Remote (mirrors @wvb/electron's remote.ts).
import { Remote, type RemoteConfig } from '../mod.ts';

export type { RemoteConfig };

export function remote(config: RemoteConfig): Remote {
  return new Remote(config);
}
