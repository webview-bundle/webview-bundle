import type { Config } from '@wvb/config';

/**
 * Minimal structural subset of electron-builder's `AfterPackContext` — only the fields this
 * integration reads. Declared locally so the package never has to import from `electron-builder` /
 * `app-builder-lib` (those are the consumer's dependency, not ours). The real `AfterPackContext`
 * is assignable to this, so the hook plugs straight into an electron-builder config.
 */
export interface AfterPackContext {
  /** Output directory for the current platform/arch build (contains the `.app` on macOS). */
  appOutDir: string;
  /** e.g. `'darwin'`, `'mas'`, `'win32'`, `'linux'`. */
  electronPlatformName: string;
  /** Target CPU arch (electron-builder's `Arch` enum value) — used to stage per target. */
  arch: number;
  packager: {
    /** The electron-builder project directory (where the config / package.json live). */
    projectDir: string;
    appInfo: {
      /** The product file name (the `.app` bundle base name on macOS). */
      productFilename: string;
    };
  };
}

/**
 * An electron-builder `afterPack` lifecycle hook. Matches the shape electron-builder invokes — it
 * is called once per platform/arch target with the build context.
 */
export type AfterPackHook = (context: AfterPackContext) => void | Promise<void>;

/**
 * Options for the webview-bundle electron-builder integration.
 *
 * These mirror `@wvb/electron-forge`'s plugin options so the two integrations expose the same knobs.
 * `root` and `builtin` are read from your webview-bundle config (`wvb.config.ts`); pass them inline
 * here to override the config file (inline values win).
 */
export interface WebViewBundleOptions extends Pick<Config, 'root' | 'builtin'> {
  /**
   * Directory name (under the packaged app's resources) that the Electron runtime resolves builtin
   * bundles from — `@wvb/electron`'s `defaultBuiltinDir()` reads `<process.resourcesPath>/bundles`.
   *
   * Only change this if you also pass a custom `builtinDir` to `bundleSource()` at runtime; the
   * basename must match.
   *
   * @default 'bundles'
   */
  bundlesDir?: string;
  /**
   * Controls loading of a webview-bundle config file (e.g. `wvb.config.ts`).
   *
   * - `true` (or omitted): auto-discover a config file from the project root and merge it with the
   *   inline options below. Inline fields take precedence.
   * - `string`: load the config file at this explicit path (merged with inline).
   * - `false`: skip config-file loading entirely and use only the inline options.
   *
   * @default true
   */
  configFile?: string | boolean;
  /**
   * Release channel to install builtin bundles from (e.g. `"beta"`, `"alpha"`). Only applies to a
   * `remote` target.
   */
  channel?: string;
  /**
   * Throw when no builtin bundles end up installed — either because no `builtin` config could be
   * resolved, or because the resolved target produced zero bundles. Set to `false` to allow a build
   * with no builtin bundles.
   *
   * @default true
   */
  throwWhenBuiltinIsEmpty?: boolean;
}
