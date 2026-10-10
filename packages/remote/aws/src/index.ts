import type { SignatureConfig } from '@wvb/config/remote';
import { type AwsDeployer, type AwsDeployerConfig, awsDeployer } from './deployer.js';
import type { AwsClientDefaults } from './sdk.js';
import { type AwsKmsSignatureSignerConfig, awsKmsSignatureSigner } from './signature.js';
import { type AwsUploader, type AwsUploaderConfig, awsUploader } from './uploader.js';

export type { AwsDeployer, AwsDeployerConfig } from './deployer.js';
export { awsDeployer } from './deployer.js';
export { BundleAlreadyUploadedError, isBundleAlreadyUploadedError } from './errors.js';
export type { AwsKmsSignatureSignerConfig } from './signature.js';
export { awsKmsSignatureSigner } from './signature.js';
export type { AwsUploader, AwsUploaderConfig } from './uploader.js';
export { awsUploader } from './uploader.js';

export interface AwsRemoteConfig {
  bucket: string;
  uploader?: Omit<AwsUploaderConfig, 'bucket'>;
  deployer?: Omit<AwsDeployerConfig, 'bucket'>;
  signature?: false | AwsKmsSignatureSignerConfig;
  aws?: AwsClientDefaults;
}

export interface AwsRemote {
  uploader: AwsUploader;
  deployer: AwsDeployer;
  signature?: SignatureConfig;
}

/**
 * AWS remote configuration.
 */
export function awsRemote(config: AwsRemoteConfig): AwsRemote {
  const uploader = awsUploader({
    bucket: config.bucket,
    ...config.uploader,
    s3ClientConfig: {
      ...config.aws,
      ...config.uploader?.s3ClientConfig,
    },
  });
  const deployer = awsDeployer({
    bucket: config.bucket,
    ...config.deployer,
    s3ClientConfig: {
      ...config.aws,
      ...config.deployer?.s3ClientConfig,
    },
    cloudFrontClientConfig: {
      ...config.aws,
      ...config.deployer?.cloudFrontClientConfig,
    },
  });
  const signature =
    config.signature != null && config.signature !== false
      ? awsKmsSignatureSigner({
          ...config.signature,
          kmsClientConfig: {
            ...config.aws,
            ...config.signature?.kmsClientConfig,
          },
        })
      : undefined;
  const remote: AwsRemote = { uploader, deployer, signature };
  return remote;
}
