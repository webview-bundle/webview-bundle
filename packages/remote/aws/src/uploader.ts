import type { Configuration as UploadConfig } from '@aws-sdk/lib-storage';
import type { BaseRemoteUploader, RemoteUploadParams } from '@wvb/config/remote';
import { type AwsS3ClientConfigLike, filterS3Metadata, getS3Client } from './utils.js';

export interface AwsS3RemoteUploaderConfig extends AwsS3ClientConfigLike {
  bucket: string;
  key?: string | ((bundleName: string, version: string) => string);
  contentType?: string;
  cacheControl?: string;
  contentDisposition?: string;
  metadata?: Record<string, string | null | undefined>;
  upload?: UploadConfig;
}

class AwsS3RemoteUploaderImpl implements BaseRemoteUploader {
  _onUploadProgress:
    | ((progress: { loaded?: number; total?: number; part?: number }) => void)
    | undefined;

  constructor(private readonly config: AwsS3RemoteUploaderConfig) {}

  async upload(params: RemoteUploadParams): Promise<void> {
    const {
      bucket,
      upload: uploaderConfig,
      contentType = 'application/webview-bundle',
      cacheControl,
      contentDisposition,
      metadata: customMetadata = {},
    } = this.config;
    const { bundle, bundleName, version, integrity, signature } = params;
    const s3 = await getS3Client(this.config);
    const metadata: Record<string, string | null | undefined> = {
      ...customMetadata,
      'webview-bundle-name': bundleName,
      'webview-bundle-version': version,
    };
    if (integrity != null) {
      metadata['webview-bundle-integrity'] = integrity;
    }
    if (signature != null) {
      metadata['webview-bundle-signature'] = signature;
    }
    const { Upload: Uploader } = await import('@aws-sdk/lib-storage');
    const uploader = new Uploader({
      client: s3,
      params: {
        Bucket: bucket,
        Key: buildKey(this.config, params),
        Body: bundle,
        ContentType: contentType,
        CacheControl: cacheControl,
        ContentDisposition: contentDisposition,
        Metadata: filterS3Metadata(metadata),
      },
      ...uploaderConfig,
    });
    uploader.on('httpUploadProgress', progress => {
      this._onUploadProgress?.(progress);
    });
    await uploader.done();
  }
}

export function awsS3RemoteUploader(config: AwsS3RemoteUploaderConfig): BaseRemoteUploader {
  return new AwsS3RemoteUploaderImpl(config);
}

function buildKey(config: AwsS3RemoteUploaderConfig, params: RemoteUploadParams): string {
  if (typeof config.key === 'string') {
    return config.key;
  }
  const { bundleName, version } = params;
  if (typeof config.key === 'function') {
    return config.key(bundleName, version);
  }
  return `bundles/${bundleName}/${bundleName}_${version}.wvb`;
}
