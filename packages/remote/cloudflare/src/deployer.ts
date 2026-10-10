import { randomUUID } from 'node:crypto';
import type { BaseDeployer, DeployParams } from '@wvb/config/remote';
import { type AwsS3ClientConfigLike, getS3Client, readS3JsonFile } from '@wvb/remote-aws';
import {
  type BundleUpdate,
  type Current,
  generateUpdateDataETag,
  generateUpdateDataSignatures,
  type Update,
  type UpdateData,
} from '@wvb/remote-base';
import {
  type CloudflareClientConfigLike,
  getCloudflareClient,
  readKVJsonValue,
  writeKVJsonValue,
} from './utils.js';

export interface CloudflareRemoteDeployerConfig
  extends CloudflareClientConfigLike,
    AwsS3ClientConfigLike {
  accountId: string;
  bucket: string;
  kvNamespaceId: string;
}
export interface CloudflareRemoteDeployer extends BaseDeployer {}

class CloudflareRemoteDeployerImpl implements CloudflareRemoteDeployer {
  constructor(private readonly config: CloudflareRemoteDeployerConfig) {}

  async deploy(params: DeployParams) {
    const { accountId, bucket, kvNamespaceId, s3ClientConfig } = this.config;
    const { bundles, channel, runtimeVersion = 1, metadata, signatures } = params;

    const client = await getCloudflareClient(this.config);
    const s3 = await getS3Client({
      ...this.config,
      s3ClientConfig: {
        ...s3ClientConfig,
        region: s3ClientConfig?.region ?? 'auto',
        endpoint: s3ClientConfig?.endpoint ?? `https://${accountId}.r2.cloudflarestorage.com`,
      },
    });

    const previousCurrent = await readKVJsonValue<Current>(client, {
      accountId,
      namespaceId: kvNamespaceId,
      key: this.#currentKey(channel, runtimeVersion),
    });
    const previousUpdate =
      previousCurrent == null
        ? null
        : await readKVJsonValue<Update>(client, {
            accountId,
            namespaceId: kvNamespaceId,
            key: this.#updateKey(channel, runtimeVersion, previousCurrent.updateId),
          });

    const updatedBundles = new Map(
      previousUpdate?.data.bundles.map(bundle => [bundle.name, bundle])
    );

    for (const bundle of bundles) {
      const versionData = await readS3JsonFile<Pick<BundleUpdate, 'integrity' | 'metadata'>>(s3, {
        bucket,
        key: `bundles/${bundle.name}/${bundle.version}.json`,
      });
      updatedBundles.set(bundle.name, { ...versionData, ...bundle });
    }

    const data: UpdateData = {
      id: randomUUID(),
      createdAt: new Date().toISOString(),
      runtimeVersion,
      bundles: [...updatedBundles.values()].sort((a, b) => a.name.localeCompare(b.name)),
      metadata: metadata ?? previousUpdate?.data.metadata,
    };

    const update: Update = {
      data,
      etag: generateUpdateDataETag(data),
      signatures: signatures != null ? await generateUpdateDataSignatures(data, signatures) : [],
    };
    await writeKVJsonValue(client, {
      accountId,
      namespaceId: kvNamespaceId,
      key: this.#updateKey(channel, runtimeVersion, update.data.id),
      data: update,
    });

    const current: Current = {
      updateId: update.data.id,
      currentAt: new Date().toISOString(),
    };
    await writeKVJsonValue(client, {
      accountId,
      namespaceId: kvNamespaceId,
      key: this.#currentKey(channel, runtimeVersion),
      data: current,
    });
  }

  #currentKey(channel: string | undefined, runtimeVersion: number): string {
    if (channel != null) {
      return `channels/${channel}/updates/${runtimeVersion}/current.json`;
    }
    return `updates/${runtimeVersion}/current.json`;
  }

  #updateKey(channel: string | undefined, runtimeVersion: number, updateId: string): string {
    if (channel != null) {
      return `channels/${channel}/updates/${runtimeVersion}/${updateId}.json`;
    }
    return `updates/${runtimeVersion}/${updateId}.json`;
  }
}

export function cloudflareRemoteDeployer(
  config: CloudflareRemoteDeployerConfig
): CloudflareRemoteDeployer {
  return new CloudflareRemoteDeployerImpl(config);
}
