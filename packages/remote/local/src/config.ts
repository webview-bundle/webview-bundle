import type { BaseDeployer, BaseUploader } from '@wvb/config/remote';
import { getDefaultBaseDir } from './base-dir.js';
import { localRemoteDeployer } from './deployer.js';
import { localRemoteUploader } from './uploader.js';

export interface LocalRemoteConfig {
  /**
   * @default "~/.wvb/local"
   */
  baseDir?: string;
}

export interface LocalRemote {
  uploader: BaseUploader;
  deployer: BaseDeployer;
}

export function localRemote(config: LocalRemoteConfig): LocalRemote {
  const resolvedConfig = {
    baseDir: config.baseDir ?? getDefaultBaseDir(),
  };

  const uploader = localRemoteUploader(resolvedConfig);
  const deployer = localRemoteDeployer(resolvedConfig);
  const remote: LocalRemote = { uploader, deployer };
  return remote;
}
