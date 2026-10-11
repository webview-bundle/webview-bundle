import type { BaseUploader, UploadParams } from '@wvb/config/remote';
import { getBundleFileSize, writeBundle, writeBundleVersionData } from './api/index.js';

export interface LocalRemoteUploaderConfig {
  baseDir: string;
}

class LocalUploader implements BaseUploader {
  constructor(private readonly config: LocalRemoteUploaderConfig) {}

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

export function localRemoteUploader(config: LocalRemoteUploaderConfig): BaseUploader {
  return new LocalUploader(config);
}
