import type { Remote, Source, Updater, UpdaterOptions } from '@wvb/node';
import { registerIpc } from './ipc.js';
import { wvbNode } from './native.js';
import { type Protocol, registerProtocol } from './protocol.js';
import { type RemoteOptions, remote } from './remote.js';
import { type SourceOptions, source } from './source.js';

/** Remote endpoint and options used by the optional updater. */
export interface WebviewBundleRemoteConfig extends RemoteOptions {
  /** Base URL of the update service. */
  baseUrl: string;
}

/** Updater configuration owned by an Electron host process. */
export interface WebviewBundleUpdaterConfig extends UpdaterOptions {
  /** Remote service from which updates are requested. */
  remote: WebviewBundleRemoteConfig;
  /** Persistent location for the last installed update metadata. */
  updateFilepath: string;
}

/** Configuration used to initialize the Electron integration. */
export interface WebviewBundleConfig {
  /** Bundle source configuration. Defaults to Electron-specific application directories. */
  source?: SourceOptions;
  /** Optional remote-update configuration. */
  updater?: WebviewBundleUpdaterConfig;
  /** Protocols to register before the instance becomes ready. */
  protocols: Protocol[];
}

/**
 * Host-process facade for bundle sources, custom protocols, and optional remote updates.
 *
 * Await {@link WebviewBundle.ready} before serving requests or exposing its IPC API to renderers.
 */
export class WebviewBundle {
  private readonly _source: Source;
  private readonly _remote: Remote | null = null;
  private readonly _updater: Updater | null = null;
  private readonly _ready: Promise<void>;

  /** Creates a host-process bundle manager and starts protocol registration. */
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

  /** Schemes registered for this instance. */
  get protocolSchemes(): readonly string[] {
    return this.config.protocols.map(x => x.scheme);
  }

  /** Local builtin and remote bundle source. */
  get source(): Source {
    return this._source;
  }

  /** Remote client, or `null` when updates were not configured. */
  get remote(): Remote | null {
    return this._remote;
  }

  /** Updater, or `null` when updates were not configured. */
  get updater(): Updater | null {
    return this._updater;
  }

  /** Resolves after all configured Electron protocols have been registered. */
  ready(): Promise<void> {
    return this._ready;
  }
}

/** Creates an Electron bundle manager and registers its renderer IPC transport. */
export function webviewBundle(config: WebviewBundleConfig): WebviewBundle {
  const instance = new WebviewBundle(config);
  registerIpc(instance);
  return instance;
}

/** Short alias for {@link webviewBundle}. */
export const wvb: typeof webviewBundle = webviewBundle;
