import { type BundleSource, loadLib, type Remote, Updater, type UpdaterOptions } from '@wvb/deno';
import type { Protocol } from './protocol.ts';
import { type RemoteOptions, remote } from './remote.ts';
import { bundleSource, type SourceOptions } from './source.ts';

export interface WebviewBundleRemoteConfig extends RemoteOptions {
  endpoint: string;
}

export interface WebviewBundleUpdaterConfig extends UpdaterOptions {
  remote: WebviewBundleRemoteConfig;
}

export interface WebviewBundleConfig {
  /**
   * Deno-specific: the native cdylib path (e.g. a `deno desktop --include`d dir resolved from the
   * app's `import.meta.url`). Omit if you already loaded it via `loadLib`/`loadLibViaPlug`.
   */
  lib?: string | URL;
  source?: SourceOptions;
  updater?: WebviewBundleUpdaterConfig;
  protocols: Protocol[];
}

async function buildHandler(
  protocols: Protocol[],
  source: BundleSource
): Promise<(req: Request) => Promise<Response>> {
  if (protocols.length === 0) {
    throw new Error('webviewBundle requires at least one protocol');
  }
  // Deno desktop is single-origin: the first protocol is served at the HTTP root.
  const [protocol] = protocols;
  const handler =
    typeof protocol.handler === 'function' ? await protocol.handler({ source }) : protocol.handler;
  const onError = protocol.options?.onError;
  return async (req: Request): Promise<Response> => {
    try {
      return await handler.handle(req);
    } catch (e) {
      const error = e instanceof Error ? e : new Error(String(e));
      onError?.(error);
      return new Response(error.message, { status: 500 });
    }
  };
}

export class WebviewBundle {
  readonly #config: WebviewBundleConfig;
  readonly #source: BundleSource;
  readonly #remote: Remote | null = null;
  readonly #updater: Updater | null = null;
  readonly #handler: Promise<(req: Request) => Promise<Response>>;

  constructor(config: WebviewBundleConfig) {
    // Fail fast on invalid config before any side effects (loading the lib, creating dirs), rather
    // than deferring the error to the first request.
    if (config.protocols.length === 0) {
      throw new Error('webviewBundle requires at least one protocol');
    }
    this.#config = config;
    if (config.lib != null) {
      loadLib(config.lib);
    }
    this.#source = bundleSource(config.source);
    if (config.updater != null) {
      const { remote: remoteConfig, ...updaterOptions } = config.updater;
      const { endpoint, ...remoteOptions } = remoteConfig;
      this.#remote = remote(endpoint, remoteOptions);
      this.#updater = new Updater(this.#source, this.#remote, updaterOptions);
    }
    this.#handler = buildHandler(config.protocols, this.#source);
  }

  get protocolSchemes(): readonly string[] {
    return this.#config.protocols.map(p => p.scheme);
  }

  get source(): BundleSource {
    return this.#source;
  }

  get remote(): Remote | null {
    return this.#remote;
  }

  get updater(): Updater | null {
    return this.#updater;
  }

  /** A `Deno.serve`-compatible handler: `Deno.serve(wvb.fetch)`. */
  fetch = (req: Request): Promise<Response> => this.#handler.then(handle => handle(req));
}

export function webviewBundle(config: WebviewBundleConfig): WebviewBundle {
  return new WebviewBundle(config);
}

export const wvb: typeof webviewBundle = webviewBundle;
