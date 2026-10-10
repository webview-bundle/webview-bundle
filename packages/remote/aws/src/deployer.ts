import { randomUUID } from 'node:crypto';
import type { BaseDeployer, DeployParams } from '@wvb/config/remote';
import {
  type BundleUpdate,
  type Current,
  generateUpdateDataETag,
  generateUpdateDataSignatures,
  type Update,
  type UpdateData,
} from '@wvb/remote-base';
import {
  type AwsCloudFrontClientConfigLike,
  type AwsS3ClientConfigLike,
  getCloudFrontClient,
  getS3Client,
  readS3JsonFile,
  writeS3JsonFile,
} from './sdk.js';

export interface AwsRemoteDeployerConfig
  extends AwsS3ClientConfigLike,
    AwsCloudFrontClientConfigLike {
  bucket: string;
  invalidate?: { distributionId: string; callerReference?: string | (() => string) };
}
export interface AwsRemoteDeployer extends BaseDeployer {}

class AwsDeployerImpl implements AwsRemoteDeployer {
  constructor(private readonly config: AwsRemoteDeployerConfig) {}

  async deploy(params: DeployParams) {
    const { bucket, invalidate } = this.config;
    const { bundles, channel, runtimeVersion = 1, metadata, signatures } = params;

    const s3 = await getS3Client(this.config);

    const previousCurrent = await readS3JsonFile<Current>(s3, {
      bucket,
      key: this.#currentKey(channel, runtimeVersion),
    });
    const previousUpdate =
      previousCurrent == null
        ? null
        : await readS3JsonFile<Update>(s3, {
            bucket,
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
    await writeS3JsonFile(s3, {
      bucket,
      key: this.#updateKey(channel, runtimeVersion, update.data.id),
      data: update,
      cacheControl: 'no-cache',
    });

    const current: Current = {
      updateId: update.data.id,
      currentAt: new Date().toISOString(),
    };
    await writeS3JsonFile(s3, {
      bucket,
      key: this.#currentKey(channel, runtimeVersion),
      data: current,
      cacheControl: 'no-cache',
    });

    if (invalidate != null) {
      const { CreateInvalidationCommand } = await import('@aws-sdk/client-cloudfront');
      const client = await getCloudFrontClient(this.config);
      const reference = invalidate.callerReference;
      await client.send(
        new CreateInvalidationCommand({
          DistributionId: invalidate.distributionId,
          InvalidationBatch: {
            CallerReference: typeof reference === 'function' ? reference() : (reference ?? data.id),
            Paths: {
              Quantity: 1,
              Items: ['/update*'],
            },
          },
        })
      );
    }
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

export function awsRemoteDeployer(config: AwsRemoteDeployerConfig): AwsRemoteDeployer {
  return new AwsDeployerImpl(config);
}
