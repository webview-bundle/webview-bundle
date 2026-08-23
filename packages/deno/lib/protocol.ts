import type { HostnameSegment, HttpMethod, UriPathResolver } from './bindings.ts';
import { cstr, getLib, type HttpResponse, readHandle, readResponse, requireHandle } from './ffi.ts';
import type { Source } from './source.ts';

export type { HostnameSegment, HttpMethod, UriPathResolver };

/**
 * How the bundle name is resolved from the request uri.
 *
 * Hand-written rather than generated, because `segment` is a union of a named segment and an index.
 */
export type UriBundleResolver =
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
 * Entries are served with the read options the {@link Source} was configured with; to change
 * data-checksum verification, set `options.dataRead` on the source.
 */
export interface BundleProtocolOptions {
  /** Default: the first hostname segment. */
  bundleResolver?: UriBundleResolver;
  /** Default: `'directory_index'`. */
  pathResolver?: UriPathResolver;
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
 * Serves files from a {@link Source} as HTTP responses — GET/HEAD, content-type, HTTP Range
 * (206), and `index.html` directory-index fallback. By default the bundle name comes from the
 * request URI host (`bundle://app/index.html` → bundle `app`); pass {@link BundleProtocolOptions}
 * to resolve the bundle name or the path differently.
 */
export class BundleProtocol {
  #ptr: Deno.PointerValue;

  constructor(source: Source, options?: BundleProtocolOptions) {
    const lib = getLib();
    // A misspelled option (`pathresolver`) is rejected by the native side rather than dropped in
    // silence, which would leave the request served with a setting the caller did not ask for.
    this.#ptr = readHandle(
      lib,
      lib.symbols.wvb_bundle_protocol_new(
        source.pointer,
        cstr(options != null ? JSON.stringify(options) : '')
      )
    );
  }

  /** Serves from the bundle; a request body is accepted but unused (only GET/HEAD are served). */
  handle(
    method: HttpMethod,
    uri: string,
    headers?: Record<string, string>,
    body?: Uint8Array<ArrayBuffer>
  ): Promise<HttpResponse> {
    return handleProtocol(requireHandle(this.#ptr, 'BundleProtocol'), method, uri, headers, body);
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
 *
 * Unlike `@wvb/node`, the resolver cannot be a JavaScript callback: a `nonblocking` FFI call runs on
 * a worker thread that cannot re-enter the JS event loop.
 */
export class ProxyProtocol {
  #ptr: Deno.PointerValue;

  constructor(resolver: Record<string, string>) {
    const lib = getLib();
    this.#ptr = readHandle(lib, lib.symbols.wvb_proxy_protocol_new(cstr(JSON.stringify(resolver))));
  }

  /** Forwards the request — including `body`, for POST/PUT/PATCH — to the resolved target. */
  handle(
    method: HttpMethod,
    uri: string,
    headers?: Record<string, string>,
    body?: Uint8Array<ArrayBuffer>
  ): Promise<HttpResponse> {
    return handleProtocol(requireHandle(this.#ptr, 'ProxyProtocol'), method, uri, headers, body);
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
