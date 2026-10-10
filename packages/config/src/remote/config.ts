import type { BundleNameResolver, VersionResolver } from '../common.js';
import type { BaseDeployer } from './deployer.js';
import type { IntegrityMakeConfig } from './integrity.js';
import type { SignatureConfig, SignatureSignConfig } from './signature.js';
import type { BaseUploader } from './uploader.js';

export interface RemoteConfig {
  /**
   * Endpoint to remote server.
   */
  endpoint?: string;
  /**
   * Name of the bundle to be used in remote.
   */
  bundleName?: BundleNameResolver;
  /**
   * Version of the bundle to be used in remote.
   */
  version?: VersionResolver;
  /**
   * Whether to pack the bundle before uploading.
   * @default true
   */
  packBeforeUpload?: boolean;
  uploader?: BaseUploader;
  deployer?: BaseDeployer;
  integrity?: boolean | IntegrityMakeConfig;
  signature?: SignatureSignConfig | SignatureConfig[];
}
