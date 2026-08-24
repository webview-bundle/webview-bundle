import type {
  BundleUpdate,
  HttpOptions,
  RemoteConfig,
  RemoteUpdateResponse,
  Update,
  UpdateSignature,
} from './bindings.ts';
import type { Cancellation } from './cancellation.ts';
import { cstr, getLib, readHandle, readJsonAsync, requireHandle } from './ffi.ts';
import { type SignatureVerifyKey, serializeSignatureVerifyKey } from './signature.ts';

export type {
  BundleUpdate,
  HttpOptions,
  RemoteConfig,
  RemoteUpdateResponse,
  Update,
  UpdateSignature,
};

export interface RemoteGetUpdateOptions {
  /** The etag of the update previously received; sent as `if-none-match`. */
  etag?: string;
  /** Release channel to request. */
  channel?: string;
  /** Require the response to be signed by this key. */
  expectSignature?: SignatureVerifyKey;
}

function serializeGetUpdateOptions(options: RemoteGetUpdateOptions): string {
  const { expectSignature, ...rest } = options;
  return JSON.stringify(
    expectSignature == null
      ? rest
      : { ...rest, expectSignature: serializeSignatureVerifyKey(expectSignature) }
  );
}

/**
 * HTTP client for the update server: fetches the update document and downloads bundle files.
 *
 * Owns a native handle — call {@link Remote.free} (or `using remote = new Remote(...)`) when done.
 */
export class Remote {
  #ptr: Deno.PointerValue;

  /** Creates a client for the configured update service. */
  constructor(config: RemoteConfig) {
    const lib = getLib();
    this.#ptr = readHandle(lib, lib.symbols.wvb_remote_new(cstr(JSON.stringify(config))));
  }

  /** @internal Native handle, for passing to an updater. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    return requireHandle(this.#ptr, 'Remote');
  }

  /** The update document, or `null` when the server answered `304 Not Modified`. */
  getUpdate(options?: RemoteGetUpdateOptions): Promise<RemoteUpdateResponse | null> {
    const lib = getLib();
    return readJsonAsync(
      lib.symbols.wvb_remote_get_update(
        this.pointer,
        cstr(options != null ? serializeGetUpdateOptions(options) : '')
      )
    );
  }

  /** Downloads `url` into `filepath`. Cancelling rejects the call with `core.cancelled`. */
  async download(url: string, filepath: string, cancellation?: Cancellation): Promise<void> {
    const lib = getLib();
    await readJsonAsync(
      lib.symbols.wvb_remote_download(
        this.pointer,
        cstr(url),
        cstr(filepath),
        cancellation?.pointer ?? null
      )
    );
  }

  /** Releases the native remote handle. Safe to call more than once. */
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
