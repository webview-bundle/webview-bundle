export type IgnoreConfig = Array<string | RegExp> | ((file: string) => boolean | Promise<boolean>);
export type HeadersConfig =
  | Record<string, HeadersInit>
  | Array<[string, HeadersInit]>
  | ((file: string) => HeadersInit | null | undefined | Promise<HeadersInit | null | undefined>);

/**
 * Webview Bundle pack config.
 */
export interface PackConfig {
  /**
   * Path to the source directory.
   *
   * All files under this directory will be included in the Webview Bundle.
   * Use "ignore" to exclude files you don't want to pack.
   *
   * @default "./dist"
   */
  srcDir?: string;
  /**
   * Directory that out-file should be created.
   * @default "./.wvb"
   */
  outDir?: string;
  /**
   * Outfile name to create Webview Bundle archive.
   * If not provided, default to name field in "package.json" with normalized.
   */
  outFileName?: string;
  /**
   * Overwrite out-file if file is already exists
   * @default true
   */
  overwrite?: boolean;
  /**
   * Ignore patterns which exclude files from the bundle.
   */
  ignore?: IgnoreConfig;
  /**
   * Headers to set for each file in the Webview Bundle.
   *
   * @example
   * {
   *   "*.html": {
   *     "cache-control": "max-age=3600",
   *   },
   *   "*.js": ["cache-control", "max-age=0"]
   * }
   */
  headers?: HeadersConfig;
}
