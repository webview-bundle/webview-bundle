import type { BaseDeployer, BaseUploader } from '@wvb/config/remote';
import type { AwsRemoteConfig } from '@wvb/remote-aws';
import { type CloudflareRemoteDeployerConfig, cloudflareRemoteDeployer } from './deployer.js';
import { type CloudflareRemoteUploaderConfig, cloudflareRemoteUploader } from './uploader.js';
import type { CloudflareClientConfigLike } from './utils.js';

export interface CloudflareRemoteConfig
  extends CloudflareClientConfigLike,
    Pick<AwsRemoteConfig, 'aws'> {
  bucket: string;
  accountId: string;
  kvNamespaceId: string;
  uploader?: Omit<CloudflareRemoteUploaderConfig, 'accountId' | 'bucket'>;
  deployer?: Omit<CloudflareRemoteDeployerConfig, 'accountId' | 'kvNamespaceId' | 'bucket'>;
}

export interface CloudflareRemote {
  uploader: BaseUploader;
  deployer: BaseDeployer;
}

export function cloudflareRemote(config: CloudflareRemoteConfig): CloudflareRemote {
  const uploader = cloudflareRemoteUploader({
    bucket: config.bucket,
    accountId: config.accountId,
    ...config.uploader,
    s3ClientConfig: {
      ...config.aws,
      ...config.uploader?.s3ClientConfig,
    },
  });
  const deployer = cloudflareRemoteDeployer({
    bucket: config.bucket,
    accountId: config.accountId,
    kvNamespaceId: config.kvNamespaceId,
    ...config.deployer,
    s3ClientConfig: { ...config.aws, ...config.deployer?.s3ClientConfig },
    cloudflare: config.deployer?.cloudflare ?? config.cloudflare,
    cloudflareConfig: {
      ...config.cloudflareConfig,
      ...config.deployer?.cloudflareConfig,
    },
  });
  const remote: CloudflareRemote = {
    uploader,
    deployer,
  };
  return remote;
}
