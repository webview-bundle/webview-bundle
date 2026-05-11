import * as os from 'node:os';
import path from 'node:path';
import type { BaseRemoteDeployer, BaseRemoteUploader } from '@wvb/config/remote';
import { localRemoteDeployer } from './deployer.js';
import { localRemoteUploader } from './uploader.js';

export interface LocalRemoteConfig {
  /**
   * @default "~/.wvb/local"
   */
  baseDir?: string;
}

export interface LocalRemote {
  uploader: BaseRemoteUploader;
  deployer: BaseRemoteDeployer;
}

export function localRemote(config: LocalRemoteConfig): LocalRemote {
  const resolvedConfig = {
    baseDir: config.baseDir ?? path.join(os.homedir(), '.wvb', 'local'),
  };

  const uploader = localRemoteUploader(resolvedConfig);
  const deployer = localRemoteDeployer(resolvedConfig);
  const remote: LocalRemote = { uploader, deployer };
  return remote;
}
