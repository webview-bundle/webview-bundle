import { WebviewBundleError } from './error.ts';
import { cstr, getLib, type HttpResponse, readResponse } from './ffi.ts';
import type { BundleSource } from './source.ts';

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
 * Serves files from a {@link BundleSource} as HTTP responses.
 */
export class BundleProtocol {
  #ptr: Deno.PointerValue;

  constructor(source: BundleSource) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_bundle_protocol_new(source.pointer);
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create BundleProtocol');
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
 * Proxies requests for custom hosts to localhost URLs (for dev servers with hot reload).
 */
export class LocalProtocol {
  #ptr: Deno.PointerValue;

  constructor(hosts: Record<string, string>) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_local_protocol_new(cstr(JSON.stringify(hosts)));
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create LocalProtocol');
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
