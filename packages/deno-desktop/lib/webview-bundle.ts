import { type BundleSource, loadLib, type Remote, Updater, type UpdaterOptions } from '@wvb/deno';
import { type RemoteOptions, remote } from './remote.ts';
import { createHandler, type Mount, normalizeRoutes, type Routes } from './routes.ts';
import { bundleSource, type SourceOptions } from './source.ts';

export interface WebviewBundleRemoteConfig extends RemoteOptions {
  endpoint: string;
}

export interface WebviewBundleUpdaterConfig extends UpdaterOptions {
  remote: WebviewBundleRemoteConfig;
}

export interface WebviewBundleConfig {
  /**
   * Deno-specific: the native cdylib path — e.g. the `.wvb/lib` directory you installed it into with
   * `deno run -A jsr:@wvb/deno/install`, or a `deno desktop --include`d dir resolved from the app's
   * `import.meta.url`. Omit if you already loaded it via `loadLib`/`loadFromGitHub`.
   */
  lib?: string | URL;
  source?: SourceOptions;
  updater?: WebviewBundleUpdaterConfig;
  /**
   * What is served at which path. Deno desktop serves a single origin over local HTTP, so bundles
   * are told apart by the request path — see {@link Routes}.
   */
  routes: Routes;
}

export class WebviewBundle {
  readonly #mounts: Mount[];
  readonly #source: BundleSource;
  readonly #remote: Remote | null = null;
  readonly #updater: Updater | null = null;
  readonly #handler: (req: Request) => Promise<Response>;

  constructor(config: WebviewBundleConfig) {
    // Fail fast on an invalid config before any side effects (loading the lib, creating dirs),
    // rather than deferring the error to the first request.
    this.#mounts = normalizeRoutes(config.routes);
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
    this.#handler = createHandler(this.#mounts, this.#source);
  }

  /** Mount paths, in match order (longest prefix first). */
  get routePaths(): readonly string[] {
    return this.#mounts.map(m => m.mountPath);
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
  fetch = (req: Request): Promise<Response> => this.#handler(req);
}

export function webviewBundle(config: WebviewBundleConfig): WebviewBundle {
  return new WebviewBundle(config);
}

export const wvb: typeof webviewBundle = webviewBundle;
