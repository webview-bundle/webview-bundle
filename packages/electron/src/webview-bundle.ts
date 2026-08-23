import type { Remote, Source, Updater, UpdaterOptions } from '@wvb/node';
import { registerIpc } from './ipc.js';
import { wvbNode } from './native.js';
import { type Protocol, registerProtocol } from './protocol.js';
import { type RemoteOptions, remote } from './remote.js';
import { type SourceOptions, source } from './source.js';

export interface WebviewBundleRemoteConfig extends RemoteOptions {
  baseUrl: string;
}

export interface WebviewBundleUpdaterConfig extends UpdaterOptions {
  remote: WebviewBundleRemoteConfig;
  /** Persistent location for the last installed update metadata. */
  updateFilepath: string;
}

export interface WebviewBundleConfig {
  source?: SourceOptions;
  updater?: WebviewBundleUpdaterConfig;
  protocols: Protocol[];
}

export class WebviewBundle {
  private readonly _source: Source;
  private readonly _remote: Remote | null = null;
  private readonly _updater: Updater | null = null;
  private readonly _ready: Promise<void>;

  constructor(private readonly config: WebviewBundleConfig) {
    this._source = source(config.source);
    if (config.updater != null) {
      const { remote: remoteConfig, updateFilepath, ...updaterOptions } = config.updater;
      const { baseUrl, ...remoteOptions } = remoteConfig;
      this._remote = remote(baseUrl, remoteOptions);
      this._updater = new wvbNode.Updater(
        this._source,
        this._remote,
        updateFilepath,
        updaterOptions
      );
    }
    this._ready = new Promise<void>((resolve, reject) => {
      Promise.all(config.protocols.map(p => registerProtocol(p, this._source)))
        .then(() => resolve())
        .catch(e => reject(e));
    });
  }

  get protocolSchemes(): readonly string[] {
    return this.config.protocols.map(x => x.scheme);
  }

  get source(): Source {
    return this._source;
  }

  get remote(): Remote | null {
    return this._remote;
  }

  get updater(): Updater | null {
    return this._updater;
  }

  ready(): Promise<void> {
    return this._ready;
  }
}

export function webviewBundle(config: WebviewBundleConfig): WebviewBundle {
  const instance = new WebviewBundle(config);
  registerIpc(instance);
  return instance;
}

export const wvb: typeof webviewBundle = webviewBundle;
