import type { BaseUploader, UploadParams } from '@wvb/config/remote';
import { type AwsRemoteUploaderConfig, awsRemoteUploader } from '@wvb/remote-aws';

export interface CloudflareRemoteUploaderConfig extends AwsRemoteUploaderConfig {
  accountId: string;
}

class CloudflareRemoteUploaderImpl implements BaseUploader {
  _onUploadProgress:
    | ((progress: { loaded?: number; total?: number; part?: number }) => void)
    | undefined;

  constructor(private readonly config: CloudflareRemoteUploaderConfig) {}

  async upload(params: UploadParams): Promise<void> {
    const { accountId, ...config } = this.config;
    const uploader = awsRemoteUploader({
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

export function cloudflareRemoteUploader(config: CloudflareRemoteUploaderConfig): BaseUploader {
  return new CloudflareRemoteUploaderImpl(config);
}
