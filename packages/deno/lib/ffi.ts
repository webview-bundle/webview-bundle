import { fromFileUrl } from '@std/path';
import { errorFromNativePayload, WebviewBundleError } from './error.ts';
import {
  fetchChecksum,
  releaseAssetName,
  releaseAssetSuffixes,
  releaseBaseUrl,
  sha256Hex,
  VERSION,
} from './release.ts';

const SYMBOLS = {
  wvb_source_new: { parameters: ['buffer', 'buffer'], result: 'pointer' },
  wvb_source_free: { parameters: ['pointer'], result: 'void' },
  // BundleSource data API (→ WvbResult). Disk/manifest ops run on the tokio runtime → nonblocking.
  wvb_source_list_bundles: { parameters: ['pointer'], result: 'pointer', nonblocking: true },
  wvb_source_load_version: {
    parameters: ['pointer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_update_version: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_resolve_filepath: {
    parameters: ['pointer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_get_builtin_filepath: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
  },
  wvb_source_get_remote_filepath: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
  },
  wvb_source_load_builtin_metadata: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_load_remote_metadata: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_unload_descriptor: { parameters: ['pointer', 'buffer'], result: 'pointer' },
  wvb_source_remove_remote_bundle: {
    parameters: ['pointer', 'buffer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_remote_retained_versions: {
    parameters: ['pointer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_source_prune_remote_bundles: {
    parameters: ['pointer', 'buffer'],
    result: 'pointer',
    nonblocking: true,
  },
  // Protocol
  wvb_bundle_protocol_new: { parameters: ['pointer', 'buffer'], result: 'pointer' },
  wvb_proxy_protocol_new: { parameters: ['buffer', 'buffer'], result: 'pointer' },
  wvb_protocol_free: { parameters: ['pointer'], result: 'void' },
  wvb_protocol_handle: {
    parameters: ['pointer', 'buffer', 'buffer', 'buffer', 'buffer', 'usize'],
    result: 'pointer',
    nonblocking: true,
  },
  wvb_response_status: { parameters: ['pointer'], result: 'u16' },
  wvb_response_headers_json: { parameters: ['pointer'], result: 'pointer' },
  wvb_response_body_ptr: { parameters: ['pointer'], result: 'pointer' },
  wvb_response_body_len: { parameters: ['pointer'], result: 'usize' },
  wvb_response_free: { parameters: ['pointer'], result: 'void' },
  // Remote
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
  // Updater
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

/** Platform cdylib filename
 * - macOS : `libwvb_deno.dylib`
 * - Linux :`libwvb_deno.so` (Linux)
 * - Windows : `wvb_deno.dll` (Windows)
 */
export function libFileName(os: typeof Deno.build.os = Deno.build.os): string {
  switch (os) {
    case 'darwin':
      return 'libwvb_deno.dylib';
    case 'windows':
      return 'wvb_deno.dll';
    default:
      return 'libwvb_deno.so';
  }
}

function resolveLibFile(libPath: string | URL): string {
  const p = libPath instanceof URL ? fromFileUrl(libPath) : libPath;
  if (/\.(dylib|so|dll)$/i.test(p)) {
    return p;
  }
  return p.endsWith('/') || p.endsWith('\\') ? `${p}${libFileName()}` : `${p}/${libFileName()}`;
}

/**
 * Load the native library from an explicit path.
 */
export function loadLib(libPath: string | URL): WvbLib {
  lib ??= Deno.dlopen(resolveLibFile(libPath), SYMBOLS);
  return lib;
}

export interface LoadLibViaPlugOptions {
  /** Release base URL (a directory) */
  url?: string;
  /** Release version, used to build the default `url` */
  version?: string;
  /**
   * Verify the download against its release `.sha256` sidecar
   * @default true
   */
  integrity?: boolean;
}

/**
 * Download the platform cdylib from a release via `@denosaurs/plug`.
 * For `deno run` / library use where the dylib isn't bundled.
 *
 * Requires `--allow-net --allow-read --allow-write --allow-env --allow-ffi`.
 */
// deno-lint-ignore require-await
export async function loadLibViaPlug(options: LoadLibViaPlugOptions = {}): Promise<WvbLib> {
  if (lib != null) {
    return lib;
  }
  loadingPromise ??= loadViaPlug(options).catch(e => {
    loadingPromise = null;
    throw e;
  });
  return loadingPromise;
}

async function loadViaPlug(options: LoadLibViaPlugOptions): Promise<WvbLib> {
  const target = Deno.build.target;
  const base = `${(options.url ?? `${releaseBaseUrl(options.version ?? VERSION)}/`).replace(/\/+$/, '')}/`;
  const { download } = await import('@denosaurs/plug');
  const path = await download({ name: 'wvb_deno', url: base, suffixes: releaseAssetSuffixes() });
  if (options.integrity !== false) {
    const checksum = await fetchChecksum(base, releaseAssetName(target));
    const actual = await sha256Hex(await Deno.readFile(path));
    if (actual !== checksum) {
      await Deno.remove(path).catch(() => {});
      throw new Error(
        `wvb: checksum mismatch for ${releaseAssetName(target)}\n  expected ${checksum}\n  actual   ${actual}`
      );
    }
  }
  if (lib != null) {
    return lib;
  }
  lib = Deno.dlopen(path, SYMBOLS) as WvbLib;
  return lib;
}

let loadingPromise: Promise<WvbLib> | null = null;

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
    'wvb: native library not loaded. Call loadLib(path), ' +
      'loadLibViaPlug({ url }), or set WVB_DENO_LIB.'
  );
}

const encoder = new TextEncoder();

/** NUL-terminate a string for passing to a `*const c_char` parameter. */
export function cstr(value: string): Uint8Array<ArrayBuffer> {
  return encoder.encode(`${value}\0`);
}

/**
 * Copy `len` bytes from a native pointer into a fresh JS-owned `Uint8Array`. Uses `copyInto` (not
 * `getArrayBuffer`) so the result never aliases Rust-owned memory.
 */
function copyBytes(ptr: Deno.PointerValue, len: number): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(len);
  if (len > 0 && ptr !== null) {
    new Deno.UnsafePointerView(ptr).copyInto(out);
  }
  return out;
}

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
    throw new WebviewBundleError('null_handle', 'wvb: native call returned a null result');
  }
  try {
    const ok = l.symbols.wvb_result_ok(resultPtr) !== 0;
    const jsonPtr = l.symbols.wvb_result_json(resultPtr);
    const text = jsonPtr === null ? '' : new Deno.UnsafePointerView(jsonPtr).getCString();
    if (!ok) {
      throw errorFromNativePayload(text);
    }
    const len = Number(l.symbols.wvb_result_body_len(resultPtr));
    const body = copyBytes(l.symbols.wvb_result_body_ptr(resultPtr), len);
    return { json: text, body };
  } finally {
    l.symbols.wvb_result_free(resultPtr);
  }
}

export function readResponse(l: WvbLib, respPtr: Deno.PointerValue): HttpResponse {
  if (respPtr === null) {
    throw new WebviewBundleError('null_handle', 'wvb: native handler returned a null response');
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
