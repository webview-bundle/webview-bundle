import type { ListRemoteBundleInfo, RemoteBundleInfo } from './bindings.ts';
import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readResult } from './ffi.ts';

export type { ListRemoteBundleInfo, RemoteBundleInfo };

export interface HttpOptions {
  /** Headers sent with every request. */
  defaultHeaders?: Record<string, string>;
  userAgent?: string;
  timeout?: number;
  readTimeout?: number;
  connectTimeout?: number;
  poolIdleTimeout?: number;
  poolMaxIdlePerHost?: number;
  referer?: boolean;
  tcpNodelay?: boolean;
  hickoryDns?: boolean;
}

export interface RemoteOptions {
  http?: HttpOptions;
}

export interface RemoteDownload {
  info: RemoteBundleInfo;
  data: Uint8Array<ArrayBuffer>;
}

/**
 * HTTP client for a remote bundle server
 */
export class Remote {
  #ptr: Deno.PointerValue;

  constructor(endpoint: string, options?: RemoteOptions) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_remote_new(
      cstr(endpoint),
      cstr(options?.http != null ? JSON.stringify(options.http) : '')
    );
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create Remote');
    }
  }

  get pointer(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: Remote has been freed');
    }
    return this.#ptr;
  }

  async listBundles(channel?: string): Promise<ListRemoteBundleInfo[]> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_remote_list_bundles(this.#ptr, cstr(channel ?? ''));
    return JSON.parse(readResult(lib, ptr).json) as ListRemoteBundleInfo[];
  }

  async getInfo(bundleName: string, channel?: string): Promise<RemoteBundleInfo> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_remote_get_info(
      this.#ptr,
      cstr(bundleName),
      cstr(channel ?? '')
    );
    return JSON.parse(readResult(lib, ptr).json) as RemoteBundleInfo;
  }

  async download(bundleName: string, channel?: string): Promise<RemoteDownload> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_remote_download(
      this.#ptr,
      cstr(bundleName),
      cstr(channel ?? '')
    );
    const { json, body } = readResult(lib, ptr);
    return { info: JSON.parse(json) as RemoteBundleInfo, data: body };
  }

  async downloadVersion(bundleName: string, version: string): Promise<RemoteDownload> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_remote_download_version(
      this.#ptr,
      cstr(bundleName),
      cstr(version)
    );
    const { json, body } = readResult(lib, ptr);
    return { info: JSON.parse(json) as RemoteBundleInfo, data: body };
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_remote_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
