import type { BundleSource, Remote, Updater, UpdaterOptions } from '@wvb/node';
import { registerIpc } from './ipc.js';
import { wvbNode } from './native.js';
import { type Protocol, registerProtocol } from './protocol.js';
import { type RemoteOptions, remote } from './remote.js';
import { bundleSource, type SourceOptions } from './source.js';

export interface WebviewBundleRemoteConfig extends RemoteOptions {
  endpoint: string;
}

export interface WebviewBundleUpdaterConfig extends UpdaterOptions {
  remote: WebviewBundleRemoteConfig;
}

export interface WebviewBundleConfig {
  source?: SourceOptions;
  updater?: WebviewBundleUpdaterConfig;
  protocols: Protocol[];
}

export class WebviewBundle {
  private readonly _source: BundleSource;
  private readonly _remote: Remote | null = null;
  private readonly _updater: Updater | null = null;
  private readonly _ready: Promise<void>;

  constructor(private readonly config: WebviewBundleConfig) {
    this._source = bundleSource(config.source);
    if (config.updater != null) {
      const { remote: remoteConfig, ...updaterOptions } = config.updater;
      const { endpoint, ...remoteOptions } = remoteConfig;
      this._remote = remote(endpoint, remoteOptions);
      this._updater = new wvbNode.Updater(this._source, this._remote, updaterOptions);
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

  get source(): BundleSource {
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

  /** @deprecated Renamed to {@link ready}. Kept as an alias for backwards compatibility. */
  whenProtocolRegistered(): Promise<void> {
    return this._ready;
  }
}

export function webviewBundle(config: WebviewBundleConfig): WebviewBundle {
  const instance = new WebviewBundle(config);
  registerIpc(instance);
  return instance;
}

export const wvb: typeof webviewBundle = webviewBundle;
