import type { BaseRemoteUploader, RemoteUploadParams } from '@wvb/config/remote';
import { type AwsS3RemoteUploaderConfig, awsS3RemoteUploader } from '@wvb/remote-aws';

export interface CloudflareRemoteUploaderConfig extends AwsS3RemoteUploaderConfig {
  accountId: string;
}

class CloudflareRemoteUploaderImpl implements BaseRemoteUploader {
  _onUploadProgress:
    | ((progress: { loaded?: number; total?: number; part?: number }) => void)
    | undefined;

  constructor(private readonly config: CloudflareRemoteUploaderConfig) {}

  async upload(params: RemoteUploadParams): Promise<void> {
    const { accountId, ...config } = this.config;
    const uploader = awsS3RemoteUploader({
      ...config,
      s3ClientConfig: {
        ...config.s3ClientConfig,
        region: config.s3ClientConfig?.region ?? 'auto',
        endpoint:
          config.s3ClientConfig?.endpoint ?? `https://${accountId}.r2.cloudflarestorage.com`,
      },
    });
    uploader._onUploadProgress = this._onUploadProgress;
    await uploader.upload(params);
  }
}

export function cloudflareRemoteUploader(
  config: CloudflareRemoteUploaderConfig
): BaseRemoteUploader {
  return new CloudflareRemoteUploaderImpl(config);
}
