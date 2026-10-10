import type { S3Client } from '@aws-sdk/client-s3';
import type { Configuration as UploadConfig } from '@aws-sdk/lib-storage';
import type { BaseUploader, UploadParams } from '@wvb/config/remote';
import { BundleAlreadyUploadedError } from './errors.js';
import {
  type AwsS3ClientConfigLike,
  filterS3Metadata,
  getS3Client,
  isNotFoundError,
} from './sdk.js';

export interface AwsRemoteUploaderConfig extends AwsS3ClientConfigLike {
  bucket: string;
  contentType?: string;
  cacheControl?: string;
  contentDisposition?: string;
  metadata?: Record<string, string | null | undefined>;
  upload?: Omit<UploadConfig, 'client' | 'params'>;
}

export interface AwsRemoteUploader extends BaseUploader {}

class AwsUploaderImpl implements AwsRemoteUploader {
  _onUploadProgress:
    | ((progress: { loaded?: number; total?: number; part?: number }) => void)
    | undefined;

  constructor(private readonly config: AwsRemoteUploaderConfig) {}

  async upload(params: UploadParams): Promise<void> {
    const {
      bucket,
      upload: uploaderConfig,
      contentType = 'application/webview-bundle',
      cacheControl,
      contentDisposition,
      metadata: customMetadata = {},
    } = this.config;
    const { bundle, name: bundleName, version, versionData } = params;

    const s3 = await getS3Client(this.config);
    await this.#ensureAbsent(s3, bundleName, version);

    const metadata: Record<string, string | null | undefined> = {
      ...customMetadata,
      'wvb-bundle-name': bundleName,
      'wvb-bundle-version': version,
    };
    const { Upload: Uploader } = await import('@aws-sdk/lib-storage');
    const uploader = new Uploader({
      client: s3,
      params: {
        Bucket: bucket,
        Key: this.#bundleKey(bundleName, version),
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
    if (versionData != null) {
      const { PutObjectCommand } = await import('@aws-sdk/client-s3');
      await s3.send(
        new PutObjectCommand({
          Bucket: bucket,
          Key: `bundles/${bundleName}/${version}.json`,
          Body: JSON.stringify(versionData),
          ContentType: 'application/json',
        })
      );
    }
  }

  #bundleKey(bundleName: string, version: string): string {
    return `bundles/${bundleName}/${version}.wvb`;
  }

  async #ensureAbsent(s3: S3Client, bundleName: string, version: string): Promise<void> {
    const { bucket } = this.config;
    const { HeadObjectCommand } = await import('@aws-sdk/client-s3');
    try {
      await s3.send(
        new HeadObjectCommand({
          Bucket: bucket,
          Key: this.#bundleKey(bundleName, version),
        })
      );
    } catch (e) {
      if (isNotFoundError(e)) {
        return;
      }
      throw e;
    }
    throw new BundleAlreadyUploadedError(bundleName, version);
  }
}

export function awsRemoteUploader(config: AwsRemoteUploaderConfig): AwsRemoteUploader {
  return new AwsUploaderImpl(config);
}
