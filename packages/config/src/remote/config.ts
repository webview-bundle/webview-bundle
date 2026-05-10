import type { BaseRemoteDeployer } from './deployer.js';
import type { IntegrityMakeConfig } from './integrity.js';
import type { SignatureSignConfig } from './signature.js';
import type { BaseRemoteUploader } from './uploader.js';

export interface RemoteConfig {
  /**
   * Endpoint to remote server.
   */
  endpoint: string;
  /**
   * Name of the bundle to be used in remote.
   */
  bundleName?: string | (() => string | Promise<string>);
  /**
   * Name of the bundle to be used in remote.
   */
  version?: string | (() => string | Promise<string>);
  uploader?: BaseRemoteUploader;
  deployer?: BaseRemoteDeployer;
  integrity?: boolean | IntegrityMakeConfig;
  signature?: SignatureSignConfig;
}
