import path from 'node:path';
import type { BundleSource, BundleSourceConfig } from '@wvb/node';
import { app } from 'electron';
import { wvbNode } from './native.js';

export interface SourceOptions extends Omit<BundleSourceConfig, 'builtinDir' | 'remoteDir'> {
  builtinDir?: string;
  remoteDir?: string;
}

export function bundleSource(options: SourceOptions = {}): BundleSource {
  const {
    builtinDir = defaultBuiltinDir(),
    remoteDir = defaultRemoteDir(),
    ...otherOptions
  } = options;
  return new wvbNode.BundleSource({
    builtinDir,
    remoteDir,
    ...otherOptions,
  });
}

function defaultBuiltinDir(): string {
  return path.join(app.isPackaged ? process.resourcesPath : process.cwd(), 'bundles');
}

function defaultRemoteDir(): string {
  return path.join(app.getPath('userData'), 'bundles');
}
