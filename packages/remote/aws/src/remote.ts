import type { SignatureConfig } from '@wvb/config/remote';
import {
  type AwsRemoteDeployer,
  type AwsRemoteDeployerConfig,
  awsRemoteDeployer,
} from './deployer.js';
import type { AwsClientDefaults } from './sdk.js';
import { type AwsKmsSignatureSignerConfig, awsKmsSignatureSigner } from './signature.js';
import {
  type AwsRemoteUploader,
  type AwsRemoteUploaderConfig,
  awsRemoteUploader,
} from './uploader.js';

export interface AwsRemoteConfig {
  bucket: string;
  uploader?: Omit<AwsRemoteUploaderConfig, 'bucket'>;
  deployer?: Omit<AwsRemoteDeployerConfig, 'bucket'>;
  signature?: false | AwsKmsSignatureSignerConfig;
  aws?: AwsClientDefaults;
}

export interface AwsRemote {
  uploader: AwsRemoteUploader;
  deployer: AwsRemoteDeployer;
  signature?: SignatureConfig[];
}

/**
 * AWS remote configuration.
 */
export function awsRemote(config: AwsRemoteConfig): AwsRemote {
  const uploader = awsRemoteUploader({
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
      ? [
          {
            sign: awsKmsSignatureSigner({
              ...config.signature,
              kmsClientConfig: {
                ...config.aws,
                ...config.signature?.kmsClientConfig,
              },
            }),
          },
        ]
      : undefined;
  const remote: AwsRemote = { uploader, deployer, signature };
  return remote;
}
