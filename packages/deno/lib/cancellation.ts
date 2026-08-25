import { WebviewBundleError } from './error.ts';
import { getLib } from './ffi.ts';

/**
 * A token handed to a download to stop it. Cancelling makes the in-flight call reject with the
 * `core.cancelled` code. Owns a native handle — call {@link Cancellation.free} (or
 * `using cancellation = new Cancellation()`) when done.
 */
export class Cancellation {
  #ptr: Deno.PointerValue;

  /** Creates a token in the active state. */
  constructor() {
    this.#ptr = getLib().symbols.wvb_cancellation_new();
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create Cancellation');
    }
  }

  /** @internal Native handle, for passing to a download. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: Cancellation has been freed');
    }
    return this.#ptr;
  }

  /** Requests cancellation of every operation using this token. */
  cancel(): void {
    getLib().symbols.wvb_cancellation_cancel(this.pointer);
  }

  /** Whether cancellation has been requested. */
  isCancelled(): boolean {
    return getLib().symbols.wvb_cancellation_is_cancelled(this.pointer) !== 0;
  }

  /** Releases the native cancellation handle. Safe to call more than once. */
  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_cancellation_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}
