import path from 'node:path';
import type { Source, SourceConfig } from '@wvb/node';
import { app } from 'electron';
import { wvbNode } from './native.js';

export interface SourceOptions extends Omit<SourceConfig, 'builtinDir' | 'remoteDir'> {
  builtinDir?: string;
  remoteDir?: string;
}

export function source(options: SourceOptions = {}): Source {
  const {
    builtinDir = defaultBuiltinDir(),
    remoteDir = defaultRemoteDir(),
    ...otherOptions
  } = options;
  return new wvbNode.Source({
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
