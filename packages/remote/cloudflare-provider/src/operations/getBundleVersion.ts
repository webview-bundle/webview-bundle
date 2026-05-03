import type { Context } from '../context.js';

export async function getBundleVersion(
  context: Context,
  bundleName: string,
  channel?: string
): Promise<string | null> {
  const key = channel != null ? `${bundleName}/${channel}` : bundleName;
  return context.kv.get(key);
}
