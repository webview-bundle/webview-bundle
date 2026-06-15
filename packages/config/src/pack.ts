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
   * Output path for the Webview Bundle archive.
   *
   * Resolved relative to the config root (or used as-is when absolute).
   * The ".wvb" extension is appended automatically when omitted.
   *
   * If not provided, defaults to ".wvb/<name>", where `<name>` is derived from
   * the "name" field in "package.json" (scope stripped).
   */
  outFile?: string;
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
