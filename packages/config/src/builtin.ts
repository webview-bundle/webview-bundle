export type RemoteBundleMatches =
  | string
  | RegExp
  | Array<string | RegExp>
  | ((info: { name: string; version: string }) => boolean | Promise<boolean>);

export interface BuiltinDownloadConfig {
  /**
   * Concurrency of the download bundles.
   */
  concurrency?: number;
}

export interface BuiltinConfig {
  /**
   * Directory path where to download builtin bundles from remote.
   * @default ".wvb/builtin/bundles"
   */
  outDir?: string;
  /**
   * Patterns to which bundles should be included from remote bundles.
   */
  include?: RemoteBundleMatches;
  /**
   * Patterns to which bundles should be excluded from remote bundles.
   */
  exclude?: RemoteBundleMatches;
  /**
   * Clean up builtin directory before the operation.
   * @default true
   */
  clean?: boolean;
  download?: BuiltinDownloadConfig;
}
