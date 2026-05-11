import type { BaseRemoteDeployer, RemoteDeployParams } from '@wvb/config/remote';
import { writeDeployment } from './api/index.js';

export interface LocalRemoteDeployerConfig {
  baseDir: string;
}

class LocalRemoteDeployer implements BaseRemoteDeployer {
  constructor(private readonly config: LocalRemoteDeployerConfig) {}

  async deploy(params: RemoteDeployParams): Promise<void> {
    const { baseDir } = this.config;
    const { bundleName, version, channel } = params;
    await writeDeployment({ baseDir, bundle: bundleName, version, channel });
  }
}

export function localRemoteDeployer(config: LocalRemoteDeployerConfig): BaseRemoteDeployer {
  return new LocalRemoteDeployer(config);
}
