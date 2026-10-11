import type { ResolvedSignatureConfig } from './signature.js';

export interface DeployBundleData {
  name: string;
  version: string;
}

export interface DeployParams {
  bundles: DeployBundleData[];
  channel?: string;
  runtimeVersion?: number;
  signatures?: ResolvedSignatureConfig[];
  metadata?: Record<string, string>;
}

export interface BaseDeployer {
  deploy(params: DeployParams): Promise<void>;
}
