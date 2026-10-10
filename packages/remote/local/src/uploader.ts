import type { BaseUploader, UploadParams } from '@wvb/config/remote';
import { getBundleFileSize, writeBundle, writeBundleVersionData } from './api/index.js';

export interface UploaderConfig {
  baseDir: string;
}

class LocalUploaderImpl implements BaseUploader {
  constructor(private readonly config: UploaderConfig) {}

  async upload(params: UploadParams): Promise<void> {
    const { baseDir } = this.config;
    const { bundle, name, version, versionData } = params;

    const bundleSize = await getBundleFileSize({
      baseDir,
      bundle: name,
      version,
    });
    if (bundleSize != null) {
      throw new Error(`Bundle already uploaded: ${version}`);
    }

    await writeBundle({ baseDir, bundle: name, version, data: bundle });
    if (versionData != null) {
      await writeBundleVersionData({
        baseDir,
        bundle: name,
        version,
        data: versionData,
      });
    }
  }
}

export function localRemoteUploader(config: UploaderConfig): BaseUploader {
  return new LocalUploaderImpl(config);
}
