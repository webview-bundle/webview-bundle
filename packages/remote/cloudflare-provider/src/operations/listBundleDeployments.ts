import type { Context } from '../context.js';
import type { RemoteBundleDeployment } from '../types.js';

export interface ListBundleDeploymentsOptions {
  limit?: number;
  cacheTtl?: number;
  cursor?: string;
}

export interface ListBundleDeploymentsResult {
  deployments: RemoteBundleDeployment[];
  nextCursor?: string;
}

const MAX_LIMIT = 100;

export async function listBundleDeployments(
  context: Context,
  options: ListBundleDeploymentsOptions = {}
): Promise<ListBundleDeploymentsResult> {
  const { limit = MAX_LIMIT, cacheTtl, cursor } = options;
  const keysResult = await context.kv.list({
    limit: Math.min(limit, MAX_LIMIT),
    cursor,
  });
  if (keysResult.keys.length === 0) {
    return { deployments: [], nextCursor: undefined };
  }
  const values = await context.kv.get(
    keysResult.keys.map(x => x.name),
    { cacheTtl }
  );
  const deployments = new Map<string, RemoteBundleDeployment>();
  for (const [versionKey, version] of values.entries()) {
    const [bundleName, channel] = versionKey.split('/');
    if (bundleName == null || version == null) {
      continue;
    }
    const current = deployments.get(bundleName) ?? { name: bundleName };
    const updated =
      channel != null
        ? {
            ...current,
            channels: {
              ...current.channels,
              [channel]: version,
            },
          }
        : {
            ...current,
            version,
          };
    deployments.set(bundleName, updated);
  }
  return {
    deployments: [...deployments.values()],
    nextCursor: keysResult.list_complete ? undefined : keysResult.cursor,
  };
}
