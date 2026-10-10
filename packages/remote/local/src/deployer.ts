import type { BaseDeployer, DeployParams } from '@wvb/config/remote';
import { writeUpdateFile } from './api/index.js';

export interface LocalRemoteDeployerConfig {
  baseDir: string;
}

class LocalRemoteDeployer implements BaseDeployer {
  constructor(private readonly config: LocalRemoteDeployerConfig) {}

  async deploy(params: DeployParams): Promise<void> {
    const { baseDir } = this.config;
    const { bundles, channel, signatures, runtimeVersion, metadata } = params;

    await writeUpdateFile({
      baseDir,
      bundles,
      channel,
      metadata,
      signatures,
      runtimeVersion,
    });
  }
}

export function localRemoteDeployer(config: LocalRemoteDeployerConfig): BaseDeployer {
  return new LocalRemoteDeployer(config);
}
