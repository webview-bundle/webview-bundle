import type { HostnameSegment, HttpMethod, PathResolver } from './bindings.ts';
import { WebviewBundleError } from './error.ts';
import { cstr, getLib, type HttpResponse, readResponse } from './ffi.ts';
import type { BundleSource } from './source.ts';

export type { HostnameSegment, HttpMethod, PathResolver };

/** How the bundle name is resolved from the request uri. */
export type BundleResolverOptions =
  | {
      type: 'hostname';
      /** Hostname segment to use, or the nth segment (default: `'first'`). */
      segment?: HostnameSegment | number;
      /** Only resolve hosts ending in `.wvb` (default: `false`). */
      allowWvbSuffixOnly?: boolean;
    }
  | {
      type: 'pathname';
      /** Path segment index, 0-based over non-empty segments (default: `0`). */
      segmentIndex?: number;
    };

/**
 * How a {@link BundleProtocol} resolves the request uri.
 *
 * Entries are served with the read options the {@link BundleSource} was configured with; to change
 * data-checksum verification, set {@link BundleSourceConfig.dataReadOptions} on the source.
 */
export interface BundleProtocolOptions {
  /** Default: the first hostname segment. */
  bundleResolver?: BundleResolverOptions;
  /** Default: `'directoryIndex'`. */
  pathResolver?: PathResolver;
}

const PROTOCOL_OPTION_KEYS: ReadonlySet<string> = new Set(['bundleResolver', 'pathResolver']);

/**
 * Serialize the options, rejecting any key the binding does not know: a misspelled `pathResolver`
 * would otherwise be dropped in silence, leaving the request served with a setting the caller did
 * not ask for.
 */
function serializeOptions(options: BundleProtocolOptions): string {
  for (const key of Object.keys(options)) {
    if (!PROTOCOL_OPTION_KEYS.has(key)) {
      throw new WebviewBundleError('unknown', `wvb: unknown BundleProtocol option '${key}'`);
    }
  }
  return JSON.stringify(options);
}

async function handleProtocol(
  ptr: Deno.PointerValue,
  method: string,
  uri: string,
  headers?: Record<string, string>,
  body?: Uint8Array<ArrayBuffer>
): Promise<HttpResponse> {
  const lib = getLib();
  const respPtr = await lib.symbols.wvb_protocol_handle(
    ptr,
    cstr(method),
    cstr(uri),
    cstr(headers != null ? JSON.stringify(headers) : ''),
    body ?? null,
    BigInt(body?.byteLength ?? 0)
  );
  return readResponse(lib, respPtr);
}

/**
 * Serves files from a {@link BundleSource} as HTTP responses — GET/HEAD, content-type, HTTP Range
 * (206), and `index.html` directory-index fallback. By default the bundle name comes from the
 * request URI host (`bundle://app/index.html` → bundle `app`); pass {@link BundleProtocolOptions}
 * to resolve the bundle name or the path differently.
 */
export class BundleProtocol {
  #ptr: Deno.PointerValue;

  constructor(source: BundleSource, options?: BundleProtocolOptions) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_bundle_protocol_new(
      source.pointer,
      cstr(options != null ? serializeOptions(options) : '')
    );
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create BundleProtocol');
    }
  }

  /** Serves from the bundle; a request body is accepted but unused (only GET/HEAD are served). */
  handle(
    method: HttpMethod,
    uri: string,
    headers?: Record<string, string>,
    body?: Uint8Array<ArrayBuffer>
  ): Promise<HttpResponse> {
    return handleProtocol(this.#ptr, method, uri, headers, body);
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_protocol_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}

/**
 * Proxies requests for custom hosts to other servers (for dev servers with hot reload). Maps a
 * custom host to a base URL — e.g. `{ app: 'http://localhost:5173' }` serves `app://app/index.html`
 * from the dev server; the path and query of the request are appended to the target.
 */
export class ProxyProtocol {
  #ptr: Deno.PointerValue;

  constructor(hosts: Record<string, string>) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_proxy_protocol_new(cstr(JSON.stringify(hosts)));
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create ProxyProtocol');
    }
  }

  /** Forwards the request — including `body`, for POST/PUT/PATCH — to the resolved target. */
  handle(
    method: HttpMethod,
    uri: string,
    headers?: Record<string, string>,
    body?: Uint8Array<ArrayBuffer>
  ): Promise<HttpResponse> {
    return handleProtocol(this.#ptr, method, uri, headers, body);
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_protocol_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
