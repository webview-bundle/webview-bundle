import type { CloudFrontClient, CloudFrontClientConfig } from '@aws-sdk/client-cloudfront';
import type { KMSClient, KMSClientConfig } from '@aws-sdk/client-kms';
import type { S3Client, S3ClientConfig } from '@aws-sdk/client-s3';

export type AwsClientDefaults = Pick<
  S3ClientConfig,
  | 'region'
  | 'profile'
  | 'defaultUserAgentProvider'
  | 'maxAttempts'
  | 'retryMode'
  | 'retryStrategy'
  | 'useArnRegion'
  | 'defaultsMode'
  | 'credentials'
  | 'endpoint'
  | 'endpointProvider'
  | 'forcePathStyle'
  | 'followRegionRedirects'
>;

export interface AwsS3ClientConfigLike {
  s3Client?: S3Client;
  s3ClientConfig?: S3ClientConfig;
}

export async function getS3Client<T extends AwsS3ClientConfigLike = AwsS3ClientConfigLike>(
  config: T
): Promise<S3Client> {
  if (config?.s3Client != null) {
    return config.s3Client;
  }
  const { S3Client: S3ClientImpl } = await import('@aws-sdk/client-s3');
  return new S3ClientImpl({ ...config.s3ClientConfig });
}

export interface AwsCloudFrontClientConfigLike {
  cloudFrontClient?: CloudFrontClient;
  cloudFrontClientConfig?: CloudFrontClientConfig;
}

export async function getCloudFrontClient<
  T extends AwsCloudFrontClientConfigLike = AwsCloudFrontClientConfigLike,
>(config: T): Promise<CloudFrontClient> {
  if (config?.cloudFrontClient != null) {
    return config.cloudFrontClient;
  }
  const { CloudFrontClient: CloudFrontClientImpl } = await import('@aws-sdk/client-cloudfront');
  return new CloudFrontClientImpl({ ...config.cloudFrontClientConfig });
}

export interface AwsKmsClientConfigLike {
  kmsClient?: KMSClient;
  kmsClientConfig?: KMSClientConfig;
}

export async function getKmsClient<T extends AwsKmsClientConfigLike = AwsKmsClientConfigLike>(
  config: T
): Promise<KMSClient> {
  if (config?.kmsClient != null) {
    return config.kmsClient;
  }
  const { KMSClient: KMSClientImpl } = await import('@aws-sdk/client-kms');
  return new KMSClientImpl({ ...config.kmsClientConfig });
}

export function isNotFoundError(e: unknown): boolean {
  if (e == null || typeof e !== 'object') {
    return false;
  }
  const err = e as { name?: string; $metadata?: { httpStatusCode?: number } };
  return (
    err.name === 'NotFound' || err.name === 'NoSuchKey' || err.$metadata?.httpStatusCode === 404
  );
}

export function filterS3Metadata(
  metadata: Record<string, string | null | undefined>
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(metadata).filter(([, value]) => value != null)
  ) as Record<string, string>;
}
