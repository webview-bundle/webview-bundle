import type { CloudFrontClient } from '@aws-sdk/client-cloudfront';
import type { S3Client } from '@aws-sdk/client-s3';
import type { BaseDeployer } from '@wvb/config/remote';
import {
  type AwsCloudFrontClientConfigLike,
  type AwsS3ClientConfigLike,
  getCloudFrontClient,
  getS3Client,
  isNotFoundError,
} from './sdk.js';

export interface AwsDeployerConfig extends AwsS3ClientConfigLike, AwsCloudFrontClientConfigLike {
  bucket: string;
  key?: string | ((bundleName: string, version: string, channel?: string) => string);
  cacheControl?: string;
  invalidate?: {
    distributionId: string;
    callerReference?: string | (() => string);
  };
}

export interface AwsDeployer extends BaseDeployer {}

class AwsDeployerImpl implements AwsDeployer {
  constructor(private readonly config: AwsDeployerConfig) {}

  async deploy(params: RemoteDeployParams): Promise<void> {
    const { bucket, key: keyInput, invalidate, cacheControl } = this.config;
    const { bundleName, version, channel } = params;
    const s3Client = await getS3Client(this.config);
    const key =
      typeof keyInput === 'string'
        ? keyInput
        : typeof keyInput === 'function'
          ? keyInput(bundleName, version, channel)
          : `bundles/${bundleName}/deployment.json`;
    const deployment: RemoteBundleDeployment = (await this.getDeployment(
      s3Client,
      bucket,
      key
    )) ?? {
      name: bundleName,
    };
    deployment.name = bundleName;
    if (channel != null) {
      deployment.channels ??= {};
      deployment.channels[channel] = version;
    } else {
      deployment.version = version;
    }
    await this.updateDeployment(s3Client, bucket, key, deployment, cacheControl);
    if (invalidate != null) {
      const cfClient = await getCloudFrontClient(this.config);
      const callerReference =
        typeof invalidate.callerReference === 'string'
          ? invalidate.callerReference
          : typeof invalidate.callerReference === 'function'
            ? invalidate.callerReference()
            : String(Date.now());
      await this.invalidateCache(cfClient, invalidate.distributionId, callerReference);
    }
  }

  private async getDeployment(
    s3Client: S3Client,
    bucket: string,
    key: string
  ): Promise<RemoteBundleDeployment | null> {
    try {
      const { GetObjectCommand } = await import('@aws-sdk/client-s3');
      const output = await s3Client.send(
        new GetObjectCommand({
          Bucket: bucket,
          Key: key,
        })
      );
      const raw = await output.Body?.transformToString('utf8');
      if (raw == null) {
        throw new Error('Response body is empty');
      }
      return JSON.parse(raw);
    } catch (e) {
      if (isNotFoundError(e)) {
        return null;
      }
      throw e;
    }
  }

  private async updateDeployment(
    s3Client: S3Client,
    bucket: string,
    key: string,
    deployment: RemoteBundleDeployment,
    cacheControl?: string
  ): Promise<void> {
    const { PutObjectCommand } = await import('@aws-sdk/client-s3');
    await s3Client.send(
      new PutObjectCommand({
        Bucket: bucket,
        Key: key,
        Body: JSON.stringify(deployment),
        ContentType: 'application/json',
        CacheControl: cacheControl,
      })
    );
  }

  private async invalidateCache(
    cfClient: CloudFrontClient,
    distributionId: string,
    callerReference: string
  ): Promise<void> {
    const { CreateInvalidationCommand } = await import('@aws-sdk/client-cloudfront');
    await cfClient.send(
      new CreateInvalidationCommand({
        DistributionId: distributionId,
        InvalidationBatch: {
          Paths: {
            Quantity: 2,
            Items: ['/update'],
          },
          CallerReference: callerReference,
        },
      })
    );
  }
}

export function awsDeployer(config: AwsDeployerConfig): AwsDeployer {
  return new AwsDeployerImpl(config);
}
