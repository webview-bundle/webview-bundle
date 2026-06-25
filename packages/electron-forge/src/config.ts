import type { Config } from '@wvb/config';

export interface WebViewBundlePluginConfig extends Pick<Config, 'root' | 'builtin'> {
  /**
   * Directory name (under the packaged app's resources) that the Electron runtime
   * resolves builtin bundles from — `@wvb/electron`'s `defaultBuiltinDir()` reads
   * `<process.resourcesPath>/bundles`.
   *
   * @default 'bundles'
   */
  bundlesDir?: string;
  /**
   * Controls loading of a webview-bundle config file (e.g. `wvb.config.ts`).
   *
   * - `true` (or omitted): auto-discover a config file from the project root and
   *   merge it with the inline config below. Inline fields take precedence.
   * - `string`: load the config file at this explicit path (merged with inline).
   * - `false`: skip config-file loading entirely and use only the inline config.
   *
   * @default true
   */
  configFile?: string | boolean;
  /**
   * Release channel to install builtin bundles from (e.g. `"beta"`, `"alpha"`).
   * Only applies to a `remote` target.
   */
  channel?: string;
  /**
   * Throw when no builtin bundles end up installed — either because no `builtin`
   * config could be resolved, or because the resolved target produced zero
   * bundles. Set to `false` to allow a build with no builtin bundles.
   *
   * @default true
   */
  throwWhenBuiltinIsEmpty?: boolean;
}
