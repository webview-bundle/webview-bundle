import type { BaseRemoteUploader, RemoteUploadParams } from '@wvb/config/remote';
import { type BundleMetadataFile, writeBundle, writeBundleMetadata } from './api/index.js';

export interface UploaderConfig {
  baseDir: string;
}

class LocalUploaderImpl implements BaseRemoteUploader {
  constructor(private readonly config: UploaderConfig) {}

  async upload(params: RemoteUploadParams): Promise<void> {
    const { baseDir } = this.config;
    const { bundle, bundleName, version, integrity, signature } = params;
    const metadata: BundleMetadataFile = {
      integrity,
      signature,
    };
    await writeBundle({ baseDir, bundle: bundleName, version, data: bundle });
    await writeBundleMetadata({ baseDir, bundle: bundleName, version, metadata });
  }
}

export function localRemoteUploader(config: UploaderConfig): BaseRemoteUploader {
  return new LocalUploaderImpl(config);
}
