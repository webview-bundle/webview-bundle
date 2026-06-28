import type { Config } from '@wvb/config';

/**
 * Electron builder's after pack context type.
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
 * An electron-builder `afterPack` lifecycle hook.
 */
export type AfterPackHook = (context: AfterPackContext) => void | Promise<void>;

/**
 * Webview bundle options.
 */
export interface WebviewBundleOptions extends Pick<Config, 'root' | 'builtin'> {
  /**
   * Directory name (under the packaged app's resources) that the Electron runtime resolves builtin
   * bundles from — `@wvb/electron`'s `defaultBuiltinDir()` reads `<process.resourcesPath>/bundles`.
   *
   * @default 'bundles'
   */
  bundlesDir?: string;
  /**
   * Webview bundle config file option.
   *
   * - `true`: auto-discover a config file from the project root and merge it with the
   *   inline options below.
   * - `string`: load the config file at this explicit path (merged with inline).
   * - `false`: skip config-file loading entirely and use only the inline options.
   *
   * @default true
   */
  configFile?: string | boolean;
  /**
   * Release channel to install builtin bundles from (e.g. `"beta"`, `"alpha"`).
   */
  channel?: string;
  /**
   * Throw when no builtin bundles end up installed.
   *
   * @default true
   */
  throwWhenBuiltinIsEmpty?: boolean;
}
