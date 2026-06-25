// BundleSource — a builtin + remote bundle source, over the native FFI binding.
import { cstr, getLib } from './ffi.ts';

export interface BundleSourceConfig {
  /** Read-only directory of builtin bundles (`manifest.json` + `<name>/<name>_<version>.wvb`). */
  builtinDir: string;
  /** Writable directory for downloaded (remote) bundles. */
  remoteDir: string;
}

/**
 * A bundle source over a `builtinDir` (read-only) and `remoteDir` (writable). Pass it to
 * {@link BundleProtocol}. Free it with `using` or `.free()` once no longer needed (the protocol
 * keeps its own reference, so the source may be freed after the protocol is created).
 */
export class BundleSource {
  #ptr: Deno.PointerValue;

  constructor(config: BundleSourceConfig) {
    const lib = getLib();
    this.#ptr = lib.symbols.wvb_source_new(cstr(config.builtinDir), cstr(config.remoteDir));
    if (this.#ptr === null) {
      throw new Error('wvb: failed to create BundleSource');
    }
  }

  /** @internal Native handle, for passing to a protocol/updater. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new Error('wvb: BundleSource has been freed');
    }
    return this.#ptr;
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_source_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
