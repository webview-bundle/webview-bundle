import type { UriPathResolver } from '@wvb/node';

export interface ServeConfig {
  /**
   * Webview Bundle file to use for serving with http server.
   * If not provided, the file at the "outFile" path is used by default.
   */
  file?: string;
  /**
   * Specify a port number on which to start the http server.
   * @default 4312
   */
  port?: number;
  /**
   * Disable log output.
   */
  silent?: boolean;
  /**
   * How the request path is resolved to an entry path of the bundle.
   *
   * - `"exact"`: use the request path as-is (only percent-decoded).
   * - `"directoryIndex"`: `/` -> `/index.html` and `/about` -> `/about/index.html`.
   *   (static-site / MPA style; e.g. Astro `format: 'directory'` / Next `trailingSlash: true`)
   * - `"htmlExtension"`: `/` -> `/index.html` and `/about` -> `/about.html`.
   *   (flat-file style; e.g. Astro `format: 'file'` / GitHub Pages / Next `trailingSlash: false`)
   *
   * @default "directoryIndex"
   */
  pathResolver?: UriPathResolver;
}
