import {
  loadLib,
  type Remote,
  type RemoteConfig,
  Source,
  Updater,
  type UpdaterOptions,
} from '@wvb/deno';
import { remote } from './remote.ts';
import { createHandler, type Mount, normalizeRoutes, type Routes } from './routes.ts';
import { type BundleSourceConfig, resolveSourceConfig } from './source.ts';

/** Remote update options owned by a Deno Desktop host. */
export interface WebviewBundleUpdaterConfig extends UpdaterOptions {
  /** Update endpoint and HTTP options. */
  remote: RemoteConfig;
  /**
   * Where the update document last received is cached. Defaults to `update.json` in the source's
   * remote directory.
   */
  updateFilepath?: string;
}

/** Configuration used to build a Deno Desktop request handler. */
export interface WebviewBundleConfig {
  /**
   * Deno-specific: the native cdylib path — e.g. the `.wvb/lib` directory you installed it into with
   * `deno run -A jsr:@wvb/deno/install`, or a `deno desktop --include`d dir resolved from the app's
   * `import.meta.url`. Omit if you already loaded it via `loadLib`/`loadFromGitHub`.
   */
  lib?: string | URL;
  /** Bundle storage configuration. */
  source?: BundleSourceConfig;
  /** Optional remote update configuration. */
  updater?: WebviewBundleUpdaterConfig;
  /**
   * What is served at which path. Deno desktop serves a single origin over local HTTP, so bundles
   * are told apart by the request path — see {@link Routes}.
   */
  routes: Routes;
}

/** Deno Desktop facade that mounts bundles and optionally manages remote updates. */
export class WebviewBundle {
  readonly #mounts: Mount[];
  readonly #source: Source;
  readonly #remote: Remote | null = null;
  readonly #updater: Updater | null = null;
  readonly #handler: (req: Request) => Promise<Response>;

  /** Validates routes and initializes the native library, source, and optional updater. */
  constructor(config: WebviewBundleConfig) {
    // Fail fast on an invalid config before any side effects (loading the lib, creating dirs),
    // rather than deferring the error to the first request.
    this.#mounts = normalizeRoutes(config.routes);
    if (config.lib != null) {
      loadLib(config.lib);
    }
    const sourceConfig = resolveSourceConfig(config.source);
    this.#source = new Source(sourceConfig);
    if (config.updater != null) {
      const { remote: remoteConfig, updateFilepath, ...updaterOptions } = config.updater;
      this.#remote = remote(remoteConfig);
      this.#updater = new Updater(
        this.#source,
        this.#remote,
        updateFilepath ?? `${sourceConfig.remoteDir}/update.json`,
        updaterOptions
      );
    }
    this.#handler = createHandler(this.#mounts, this.#source);
  }

  /** Mount paths, in match order (longest prefix first). */
  get routePaths(): readonly string[] {
    return this.#mounts.map(m => m.mountPath);
  }

  /** Local builtin and remote bundle source. */
  get source(): Source {
    return this.#source;
  }

  /** Remote client, or `null` when updates are not configured. */
  get remote(): Remote | null {
    return this.#remote;
  }

  /** Updater, or `null` when updates are not configured. */
  get updater(): Updater | null {
    return this.#updater;
  }

  /** A `Deno.serve`-compatible handler: `Deno.serve(wvb.fetch)`. */
  fetch = (req: Request): Promise<Response> => this.#handler(req);
}

/** Creates a Deno.serve-compatible Webview Bundle host. */
export function webviewBundle(config: WebviewBundleConfig): WebviewBundle {
  return new WebviewBundle(config);
}

/** Short alias for {@link webviewBundle}. */
export const wvb: typeof webviewBundle = webviewBundle;
