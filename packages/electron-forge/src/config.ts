import type { Config } from '@wvb/config';

export interface WebviewBundlePluginConfig extends Pick<Config, 'root' | 'builtin'> {
  /**
   * Directory name (under the packaged app's resources) that the Electron runtime
   * resolves builtin bundles from.
   *
   * @default 'bundles'
   */
  bundlesDir?: string;
  /**
   * Webview bundle config file option.
   *
   * - `true`: auto-discover a config file from the project root and
   *   merge it with the inline config below. Inline fields take precedence.
   * - `string`: load the config file at this explicit path (merged with inline).
   * - `false`: skip config-file loading entirely and use only the inline config.
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
