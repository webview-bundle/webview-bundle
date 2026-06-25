// Low-level Deno FFI loader for the `wvb-deno` cdylib. Internal to the package.
import { fromFileUrl } from '@std/path';

const SYMBOLS = {
  wvb_source_new: { parameters: ['buffer', 'buffer'], result: 'pointer' },
  wvb_source_free: { parameters: ['pointer'], result: 'void' },
  wvb_bundle_protocol_new: { parameters: ['pointer'], result: 'pointer' },
  wvb_local_protocol_new: { parameters: ['buffer'], result: 'pointer' },
  wvb_protocol_free: { parameters: ['pointer'], result: 'void' },
  // nonblocking: runs on a dedicated thread (Rust block_on) so the event loop never stalls.
  wvb_protocol_handle: {
    parameters: ['pointer', 'buffer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_response_status: { parameters: ['pointer'], result: 'u16' },
  wvb_response_headers_json: { parameters: ['pointer'], result: 'pointer' },
  wvb_response_body_ptr: { parameters: ['pointer'], result: 'pointer' },
  wvb_response_body_len: { parameters: ['pointer'], result: 'usize' },
  wvb_response_free: { parameters: ['pointer'], result: 'void' },
  // Remote (network → nonblocking)
  wvb_remote_new: { parameters: ['buffer', 'buffer'], result: 'pointer' },
  wvb_remote_free: { parameters: ['pointer'], result: 'void' },
  wvb_remote_list_bundles: {
    parameters: ['pointer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_remote_get_info: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_remote_download: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_remote_download_version: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  // Updater (network → nonblocking)
  wvb_updater_new: { parameters: ['pointer', 'pointer', 'buffer'], result: 'pointer' },
  wvb_updater_free: { parameters: ['pointer'], result: 'void' },
  wvb_updater_list_remotes: { parameters: ['pointer'], result: 'pointer', nonblocking: true },
  wvb_updater_get_update: {
    parameters: ['pointer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_updater_download: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_updater_install: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  // WvbResult accessors
  wvb_result_ok: { parameters: ['pointer'], result: 'u8' },
  wvb_result_json: { parameters: ['pointer'], result: 'pointer' },
  wvb_result_body_ptr: { parameters: ['pointer'], result: 'pointer' },
  wvb_result_body_len: { parameters: ['pointer'], result: 'usize' },
  wvb_result_free: { parameters: ['pointer'], result: 'void' },
} as const;

export type WvbLib = Deno.DynamicLibrary<typeof SYMBOLS>;

let lib: WvbLib | null = null;

/** Platform cdylib filename: `libwvb_deno.dylib` (macOS) / `libwvb_deno.so` (Linux) / `wvb_deno.dll` (Windows). */
export function platformLibFileName(os: typeof Deno.build.os = Deno.build.os): string {
  const ext = os === 'windows' ? 'dll' : os === 'darwin' ? 'dylib' : 'so';
  const prefix = os === 'windows' ? '' : 'lib';
  return `${prefix}wvb_deno.${ext}`;
}

function resolveLibFile(libPath: string | URL): string {
  const p = libPath instanceof URL ? fromFileUrl(libPath) : libPath;
  // Already a dylib file → use as-is; a directory (or trailing slash) → append the platform filename.
  // Case-insensitive so explicit paths with uppercase extensions (e.g. `.DLL`) aren't mangled.
  if (/\.(dylib|so|dll)$/i.test(p)) {
    return p;
  }
  return p.endsWith('/') || p.endsWith('\\')
    ? `${p}${platformLibFileName()}`
    : `${p}/${platformLibFileName()}`;
}

/**
 * Load the native library from an explicit path and cache it (idempotent — first load wins).
 * `libPath` may be the dylib file itself or a directory containing the platform-named dylib.
 *
 * For a `deno desktop` / `deno compile` app, resolve the path from the *app's* `import.meta.url`
 * (e.g. `new URL('./vendor/wvb/', import.meta.url)`) so the `--include`d, runtime-unpacked file is
 * found. For local dev, point it at the cargo-built dylib (or set `WVB_DENO_LIB`).
 */
export function loadLib(libPath: string | URL): WvbLib {
  lib ??= Deno.dlopen(resolveLibFile(libPath), SYMBOLS);
  return lib;
}

/**
 * Download the platform cdylib from a release `url` via `@denosaurs/plug`, cache it, and load it.
 * For `deno run` / library use where the dylib isn't bundled. NOT for self-contained `deno desktop`
 * builds — there, vendor + `--include` the dylib and use {@link loadLib}.
 */
export async function loadLibViaPlug(options: { url: string }): Promise<WvbLib> {
  if (lib != null) {
    return lib;
  }
  // Single-flight: concurrent callers share one in-flight load so the dylib is opened exactly once.
  loadingPromise ??= (async () => {
    const { dlopen } = await import('@denosaurs/plug');
    lib = (await dlopen({ name: 'wvb_deno', url: options.url }, SYMBOLS)) as WvbLib;
    return lib;
  })();
  return loadingPromise;
}

let loadingPromise: Promise<WvbLib> | null = null;

/** The cached native library. Falls back to `WVB_DENO_LIB` if set; otherwise throws. */
export function getLib(): WvbLib {
  if (lib != null) {
    return lib;
  }
  // Reading env requires `--allow-env`; treat a denied/missing permission as "no override" rather
  // than crashing with a PermissionDenied.
  let env: string | undefined;
  try {
    env = Deno.env.get('WVB_DENO_LIB');
  } catch {
    env = undefined;
  }
  if (env != null && env.length > 0) {
    return loadLib(env);
  }
  throw new Error(
    'wvb: native library not loaded. Call loadLib(path) (deno desktop / dev), ' +
      'loadLibViaPlug({ url }) (deno run), or set WVB_DENO_LIB.'
  );
}

const encoder = new TextEncoder();

/** NUL-terminate a string for passing to a `*const c_char` parameter. */
export function cstr(value: string): Uint8Array<ArrayBuffer> {
  return encoder.encode(`${value}\0`);
}

/**
 * Copy `len` bytes from a native pointer into a fresh JS-owned `Uint8Array`. Uses `copyInto` (not
 * `getArrayBuffer`) so the result never aliases Rust-owned memory — the native buffer is freed right
 * after this returns, so a view would be a use-after-free.
 */
function copyBytes(ptr: Deno.PointerValue, len: number): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(len);
  if (len > 0 && ptr !== null) {
    new Deno.UnsafePointerView(ptr).copyInto(out);
  }
  return out;
}

/** A served HTTP response (mirrors `@wvb/node`'s `HttpResponse`, with `Uint8Array` for the body). */
export interface HttpResponse {
  status: number;
  headers: Record<string, string>;
  body: Uint8Array<ArrayBuffer>;
}

/** A decoded data-API result: parsed JSON payload + optional body bytes. */
export interface WvbResultData {
  json: string;
  body: Uint8Array<ArrayBuffer>;
}

/**
 * Read a `WvbResult` pointer, then free it. Throws (with the native message) when the result is an
 * error. On success returns the raw JSON string + body bytes for the caller to parse.
 */
export function readResult(l: WvbLib, resultPtr: Deno.PointerValue): WvbResultData {
  if (resultPtr === null) {
    throw new Error('wvb: native call returned a null result');
  }
  try {
    const ok = l.symbols.wvb_result_ok(resultPtr) !== 0;
    const jsonPtr = l.symbols.wvb_result_json(resultPtr);
    const text = jsonPtr === null ? '' : new Deno.UnsafePointerView(jsonPtr).getCString();
    if (!ok) {
      throw new Error(text.length > 0 ? text : 'wvb: operation failed');
    }
    const len = Number(l.symbols.wvb_result_body_len(resultPtr));
    const body = copyBytes(l.symbols.wvb_result_body_ptr(resultPtr), len);
    return { json: text, body };
  } finally {
    l.symbols.wvb_result_free(resultPtr);
  }
}

/** Read a `WvbResponse` pointer into a plain object, then free the native response. */
export function readResponse(l: WvbLib, respPtr: Deno.PointerValue): HttpResponse {
  if (respPtr === null) {
    throw new Error('wvb: native handler returned a null response');
  }
  try {
    const status = l.symbols.wvb_response_status(respPtr);
    const headersPtr = l.symbols.wvb_response_headers_json(respPtr);
    const headersJson =
      headersPtr === null ? '{}' : new Deno.UnsafePointerView(headersPtr).getCString();
    const len = Number(l.symbols.wvb_response_body_len(respPtr));
    const body = copyBytes(l.symbols.wvb_response_body_ptr(respPtr), len);
    return { status, headers: JSON.parse(headersJson) as Record<string, string>, body };
  } finally {
    l.symbols.wvb_response_free(respPtr);
  }
}
