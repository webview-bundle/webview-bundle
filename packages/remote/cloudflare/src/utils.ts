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

export async function readKVJsonValue<T = unknown>(
  client: Cloudflare,
  params: { accountId: string; namespaceId: string; key: string }
): Promise<T | null> {
  try {
    const response = await client.kv.namespaces.values.get(params.namespaceId, params.key, {
      account_id: params.accountId,
    });
    return (await response.json()) as T;
  } catch (error) {
    if (error != null && typeof error === 'object' && 'status' in error && error.status === 404) {
      return null;
    }
    throw error;
  }
}

export async function writeKVJsonValue(
  client: Cloudflare,
  params: { accountId: string; namespaceId: string; key: string; data: unknown }
): Promise<void> {
  await client.kv.namespaces.values.update(params.namespaceId, params.key, {
    account_id: params.accountId,
    value: JSON.stringify(params.data),
  });
}
