import type Cloudflare from 'cloudflare';
import type { ClientOptions } from 'cloudflare';

export interface CloudflareClientConfigLike {
  cloudflare?: Cloudflare;
  cloudflareConfig?: ClientOptions;
}

export async function getCloudflareClient<
  T extends CloudflareClientConfigLike = CloudflareClientConfigLike,
>(config: T): Promise<Cloudflare> {
  if (config.cloudflare != null) {
    return config.cloudflare;
  }
  const { default: Client } = await import('cloudflare');
  return new Client({ ...config.cloudflareConfig });
}
