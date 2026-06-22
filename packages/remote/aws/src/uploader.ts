import type { S3Client } from '@aws-sdk/client-s3';
import type { Configuration as UploadConfig } from '@aws-sdk/lib-storage';
import type { BaseRemoteUploader, RemoteUploadParams } from '@wvb/config/remote';
import { BundleAlreadyUploadedError } from './errors.js';
import {
  type AwsS3ClientConfigLike,
  filterS3Metadata,
  getS3Client,
  isNotFoundError,
} from './utils.js';

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
    const { bundle, bundleName, version, force, integrity, signature } = params;
    const s3 = await getS3Client(this.config);
    const key = buildKey(this.config, params);
    if (!force) {
      await ensureObjectAbsent(s3, bucket, key, bundleName, version);
    }
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
        Key: key,
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

async function ensureObjectAbsent(
  s3: S3Client,
  bucket: string,
  key: string,
  bundleName: string,
  version: string
): Promise<void> {
  const { HeadObjectCommand } = await import('@aws-sdk/client-s3');
  try {
    await s3.send(new HeadObjectCommand({ Bucket: bucket, Key: key }));
  } catch (e) {
    if (isNotFoundError(e)) {
      return;
    }
    throw e;
  }
  throw new BundleAlreadyUploadedError(bundleName, version);
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
