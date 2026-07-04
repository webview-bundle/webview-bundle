// BundleProtocol / LocalProtocol — resolve a request to served bundle content (or proxy to a dev
// server), plus the `HttpResponse` → web `Response` adapter.
import { cstr, getLib, type HttpResponse, readResponse } from './ffi.ts';
import type { BundleSource } from './source.ts';

/** HTTP method accepted by a protocol handler (case-insensitive on the wire). */
export type HttpMethod = 'get' | 'head' | 'options' | 'post' | 'put' | 'patch' | 'delete';

async function handleProtocol(
  ptr: Deno.PointerValue,
  method: string,
  uri: string,
  headers?: Record<string, string>
): Promise<HttpResponse> {
  const lib = getLib();
  const respPtr = await lib.symbols.wvb_protocol_handle(
    ptr,
    cstr(method),
    cstr(uri),
    cstr(headers != null ? JSON.stringify(headers) : '')
  );
  return readResponse(lib, respPtr);
}

/**
 * Serves files from a {@link BundleSource} as HTTP responses — GET/HEAD, content-type, HTTP Range
 * (206), and `index.html` directory-index fallback. Resolves the bundle name from the request URI
 * host (`bundle://app/index.html` → bundle `app`).
 */
export class BundleProtocol {
  #ptr: Deno.PointerValue;

  constructor(source: BundleSource) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_bundle_protocol_new(source.pointer);
    if (this.#ptr === null) {
      throw new Error('wvb: failed to create BundleProtocol');
    }
  }

  handle(method: HttpMethod, uri: string, headers?: Record<string, string>): Promise<HttpResponse> {
    return handleProtocol(this.#ptr, method, uri, headers);
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
 * Proxies requests for custom hosts to localhost URLs (for dev servers with hot reload). Maps a
 * custom host to a base URL — e.g. `{ app: 'http://localhost:5173' }` serves `app://app/index.html`
 * from the dev server.
 */
export class LocalProtocol {
  #ptr: Deno.PointerValue;

  constructor(hosts: Record<string, string>) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_local_protocol_new(cstr(JSON.stringify(hosts)));
    if (this.#ptr === null) {
      throw new Error('wvb: failed to create LocalProtocol');
    }
  }

  handle(method: HttpMethod, uri: string, headers?: Record<string, string>): Promise<HttpResponse> {
    return handleProtocol(this.#ptr, method, uri, headers);
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

/** Convert an {@link HttpResponse} to a web `Response` (for use inside a `Deno.serve` handler). */
export function toResponse(res: HttpResponse): Response {
  const headers = new Headers();
  for (const [name, value] of Object.entries(res.headers)) {
    headers.set(name, value);
  }
  return new Response(res.body, { status: res.status, headers });
}
