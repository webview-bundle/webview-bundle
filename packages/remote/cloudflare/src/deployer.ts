import type { BaseRemoteDeployer, RemoteDeployParams } from '@wvb/config/remote';
import { type CloudflareClientConfigLike, getCloudflareClient } from './utils.js';

export interface CloudflareRemoteDeployerConfig extends CloudflareClientConfigLike {
  accountId: string;
  kvNamespaceId: string;
  key?: string | ((bundleName: string, version: string, channel?: string) => string);
  expiration?: number;
  expirationTtl?: number;
  metadata?: unknown;
}

class CloudflareRemoteDeployerImpl implements BaseRemoteDeployer {
  constructor(private readonly config: CloudflareRemoteDeployerConfig) {}

  async deploy(params: RemoteDeployParams): Promise<void> {
    const { kvNamespaceId, key, accountId, expiration, expirationTtl, metadata } = this.config;
    const { version } = params;
    const client = await getCloudflareClient(this.config);
    await client.kv.namespaces.values.update(kvNamespaceId, this.getKey(params, key), {
      account_id: accountId,
      value: version,
      expiration,
      expiration_ttl: expirationTtl,
      metadata,
    });
  }

  private getKey(params: RemoteDeployParams, key: CloudflareRemoteDeployerConfig['key']): string {
    if (typeof key === 'string') {
      return key;
    }
    const { bundleName, version, channel } = params;
    if (typeof key === 'function') {
      return key(bundleName, version, channel);
    }
    if (channel != null) {
      return `${bundleName}/${channel}`;
    }
    return bundleName;
  }
}

export function cloudflareRemoteDeployer(
  config: CloudflareRemoteDeployerConfig
): BaseRemoteDeployer {
  return new CloudflareRemoteDeployerImpl(config);
}
