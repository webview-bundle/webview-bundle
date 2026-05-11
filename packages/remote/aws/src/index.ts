import type { BaseRemoteDeployer, BaseRemoteUploader, SignatureSignFn } from '@wvb/config/remote';
import { type AwsRemoteDeployerConfig, awsRemoteDeployer } from './deployer.js';
import { type AwsKmsSignatureSignerConfig, awsKmsSignatureSigner } from './signature.js';
import { type AwsS3RemoteUploaderConfig, awsS3RemoteUploader } from './uploader.js';
import type { AwsClientDefaults } from './utils.js';

export type { AwsRemoteDeployerConfig } from './deployer.js';
export { awsRemoteDeployer } from './deployer.js';
export type { AwsKmsSignatureSignerConfig } from './signature.js';
export { awsKmsSignatureSigner } from './signature.js';
export type { AwsS3RemoteUploaderConfig } from './uploader.js';
export { awsS3RemoteUploader } from './uploader.js';

export interface AwsRemoteConfig {
  bucket: string;
  uploader?: Omit<AwsS3RemoteUploaderConfig, 'bucket'>;
  deployer?: Omit<AwsRemoteDeployerConfig, 'bucket'>;
  signature?: false | AwsKmsSignatureSignerConfig;
  aws?: AwsClientDefaults;
}

export interface AwsRemote {
  uploader: BaseRemoteUploader;
  deployer: BaseRemoteDeployer;
  signature?: SignatureSignFn;
}

/**
 * AWS remote configuration.
 */
export function awsRemote(config: AwsRemoteConfig): AwsRemote {
  const uploader = awsS3RemoteUploader({
    bucket: config.bucket,
    ...config.uploader,
    s3ClientConfig: {
      ...config.aws,
      ...config.uploader?.s3ClientConfig,
    },
  });
  const deployer = awsRemoteDeployer({
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
