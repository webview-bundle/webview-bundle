import type {
  BundleBuilderOptions,
  BundleHeader,
  ChecksumWriteOptions,
  HeaderWriterOptions,
  IndexEntry,
  IndexWriterOptions,
  Version,
} from './bindings.ts';
import { WebviewBundleError } from './error.ts';
import { cstr, getLib, readHandle, readResult } from './ffi.ts';

export type {
  BundleBuilderOptions,
  BundleHeader,
  ChecksumWriteOptions,
  HeaderWriterOptions,
  IndexEntry,
  IndexWriterOptions,
  Version,
};

/** A bundle index: file path → its {@link IndexEntry} metadata. */
export type BundleIndex = Record<string, IndexEntry>;

// Web content types keyed by extension — the fallback when `insertEntry` is called without an
// explicit `contentType`. (The native side sniffs nothing; it stores whatever type it is given.)
const CONTENT_TYPES: Readonly<Record<string, string>> = {
  html: 'text/html',
  htm: 'text/html',
  css: 'text/css',
  js: 'text/javascript',
  mjs: 'text/javascript',
  json: 'application/json',
  jsonld: 'application/ld+json',
  svg: 'image/svg+xml',
  ico: 'image/vnd.microsoft.icon',
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  avif: 'image/avif',
  wasm: 'application/wasm',
  woff: 'font/woff',
  woff2: 'font/woff2',
  ttf: 'font/ttf',
  otf: 'font/otf',
  mp4: 'video/mp4',
  webm: 'video/webm',
  csv: 'text/csv',
  txt: 'text/plain',
  rtf: 'application/rtf',
  xml: 'application/xml',
  map: 'application/json',
};

/** Guess a content type from a path's extension, defaulting to `application/octet-stream`. */
export function contentTypeForPath(path: string): string {
  const dot = path.lastIndexOf('.');
  const ext = dot >= 0 ? path.slice(dot + 1).toLowerCase() : '';
  return CONTENT_TYPES[ext] ?? 'application/octet-stream';
}

/**
 * A `.wvb` bundle loaded into memory: its header, index and every entry's (compressed) data. Use it
 * to read multiple entries without re-parsing, or as the input to {@link writeBundle}. Owns a native
 * handle — call {@link Bundle.free} (or `using bundle = ...`) when done.
 */
export class Bundle {
  #ptr: Deno.PointerValue;

  /** @internal Use {@link readBundle}, {@link readBundleFromBytes}, {@link BundleBuilder.build}, or a {@link Source} fetch. */
  constructor(ptr: Deno.PointerValue) {
    this.#ptr = ptr;
  }

  /** @internal Native handle. Throws if already freed. */
  get pointer(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: Bundle has been freed');
    }
    return this.#ptr;
  }

  /** Decompressed contents of the entry at `path`, or `null` if it does not exist. */
  getData(path: string): Uint8Array<ArrayBuffer> | null {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_get_data(this.pointer, cstr(path));
    const { json, body } = readResult(lib, ptr);
    return JSON.parse(json) === null ? null : body;
  }

  /** The stored xxHash-32 checksum of the entry at `path`, or `null` if it does not exist. */
  getDataChecksum(path: string): number | null {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_get_data_checksum(this.pointer, cstr(path));
    return JSON.parse(readResult(lib, ptr).json) as number | null;
  }

  /** The bundle header (format version + index geometry). */
  header(): BundleHeader {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_header(this.pointer);
    return JSON.parse(readResult(lib, ptr).json) as BundleHeader;
  }

  /** The bundle index: every file path mapped to its {@link IndexEntry}. */
  index(): BundleIndex {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_index(this.pointer);
    return JSON.parse(readResult(lib, ptr).json) as BundleIndex;
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_bundle_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}

/**
 * Builds a {@link Bundle} from a set of entries. Owns a native handle — call {@link BundleBuilder.free}
 * (or `using builder = new BundleBuilder()`) when done.
 */
export class BundleBuilder {
  #ptr: Deno.PointerValue;

  constructor() {
    this.#ptr = getLib().symbols.wvb_bundle_builder_new();
    if (this.#ptr === null) {
      throw new WebviewBundleError('unknown', 'wvb: failed to create BundleBuilder');
    }
  }

  get #handle(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: BundleBuilder has been freed');
    }
    return this.#ptr;
  }

  /**
   * Adds or replaces the entry at `path`. `contentType` defaults to a type guessed from the path's
   * extension ({@link contentTypeForPath}). Returns `true` when an existing entry was replaced.
   */
  insertEntry(
    path: string,
    data: Uint8Array<ArrayBuffer>,
    contentType?: string,
    headers?: Record<string, string>
  ): boolean {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_builder_insert_entry(
      this.#handle,
      cstr(path),
      data,
      BigInt(data.byteLength),
      cstr(contentType ?? contentTypeForPath(path)),
      cstr(headers != null ? JSON.stringify(headers) : '')
    );
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  /** Removes the entry at `path`. Returns `true` when an entry existed and was removed. */
  removeEntry(path: string): boolean {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_builder_remove_entry(this.#handle, cstr(path));
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  /** Whether an entry exists at `path`. */
  containsEntry(path: string): boolean {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_builder_contains_entry(this.#handle, cstr(path));
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  /** All entry paths currently in the builder. */
  entryPaths(): string[] {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_builder_entry_paths(this.#handle);
    return JSON.parse(readResult(lib, ptr).json) as string[];
  }

  /** Builds the bundle from the current entries. */
  build(options?: BundleBuilderOptions): Bundle {
    const lib = getLib();
    const ptr = lib.symbols.wvb_bundle_builder_build(
      this.#handle,
      cstr(options != null ? JSON.stringify(options) : '')
    );
    return new Bundle(readHandle(lib, ptr));
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_bundle_builder_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}

/**
 * A bundle's metadata (header + index) without its data, backed by a `.wvb` file on disk. Reads
 * entry data lazily via {@link BundleDescriptor.getData}, reopening the file each call. Owns a native
 * handle — call {@link BundleDescriptor.free} when done.
 */
export class BundleDescriptor {
  #ptr: Deno.PointerValue;

  /** @internal Use {@link Source.fetchDescriptor}. */
  constructor(ptr: Deno.PointerValue) {
    this.#ptr = ptr;
  }

  get #handle(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: BundleDescriptor has been freed');
    }
    return this.#ptr;
  }

  /** Decompressed contents of the entry at `path`, read from `filepath`, or `null` if absent. */
  async getData(filepath: string, path: string): Promise<Uint8Array<ArrayBuffer> | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_descriptor_get_data(this.#handle, cstr(filepath), cstr(path));
    const { json, body } = readResult(lib, ptr);
    return JSON.parse(json) === null ? null : body;
  }

  /** The stored checksum of the entry at `path`, read from `filepath`, or `null` if absent. */
  async getDataChecksum(filepath: string, path: string): Promise<number | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_descriptor_get_data_checksum(
      this.#handle,
      cstr(filepath),
      cstr(path)
    );
    return JSON.parse(readResult(lib, ptr).json) as number | null;
  }

  header(): BundleHeader {
    const lib = getLib();
    const ptr = lib.symbols.wvb_descriptor_header(this.#handle);
    return JSON.parse(readResult(lib, ptr).json) as BundleHeader;
  }

  index(): BundleIndex {
    const lib = getLib();
    const ptr = lib.symbols.wvb_descriptor_index(this.#handle);
    return JSON.parse(readResult(lib, ptr).json) as BundleIndex;
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_descriptor_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}

/**
 * A cached descriptor from {@link Source.load}, pinned to a specific bundle file and
 * its read options. Its {@link LoadedDescriptor.getData} needs no filepath (it remembers one) and
 * keeps working across active-version swaps. Owns a native handle — call {@link LoadedDescriptor.free}.
 */
export class LoadedDescriptor {
  #ptr: Deno.PointerValue;

  /** @internal Use {@link Source.load}. */
  constructor(ptr: Deno.PointerValue) {
    this.#ptr = ptr;
  }

  get #handle(): Deno.PointerValue {
    if (this.#ptr === null) {
      throw new WebviewBundleError('null_handle', 'wvb: LoadedDescriptor has been freed');
    }
    return this.#ptr;
  }

  /** Decompressed contents of the entry at `path`, or `null` if it does not exist. */
  async getData(path: string): Promise<Uint8Array<ArrayBuffer> | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_loaded_descriptor_get_data(this.#handle, cstr(path));
    const { json, body } = readResult(lib, ptr);
    return JSON.parse(json) === null ? null : body;
  }

  /** The stored checksum of the entry at `path`, or `null` if it does not exist. */
  async getDataChecksum(path: string): Promise<number | null> {
    const lib = getLib();
    const ptr = await lib.symbols.wvb_loaded_descriptor_get_data_checksum(this.#handle, cstr(path));
    return JSON.parse(readResult(lib, ptr).json) as number | null;
  }

  header(): BundleHeader {
    const lib = getLib();
    const ptr = lib.symbols.wvb_loaded_descriptor_header(this.#handle);
    return JSON.parse(readResult(lib, ptr).json) as BundleHeader;
  }

  index(): BundleIndex {
    const lib = getLib();
    const ptr = lib.symbols.wvb_loaded_descriptor_index(this.#handle);
    return JSON.parse(readResult(lib, ptr).json) as BundleIndex;
  }

  free(): void {
    if (this.#ptr !== null) {
      getLib().symbols.wvb_loaded_descriptor_free(this.#ptr);
      this.#ptr = null;
    }
  }

  [Symbol.dispose](): void {
    this.free();
  }
}

/** Reads a bundle from a `.wvb` file into memory. */
export async function readBundle(filepath: string): Promise<Bundle> {
  const lib = getLib();
  const ptr = await lib.symbols.wvb_read_bundle(cstr(filepath));
  return new Bundle(readHandle(lib, ptr));
}

/** Parses a bundle from its bytes (e.g. a {@link Remote} download body). */
export function readBundleFromBytes(data: Uint8Array<ArrayBuffer>): Bundle {
  const lib = getLib();
  const ptr = lib.symbols.wvb_read_bundle_from_bytes(data, BigInt(data.byteLength));
  return new Bundle(readHandle(lib, ptr));
}

/** Writes a bundle to a `.wvb` file, returning the number of bytes written. */
export async function writeBundle(bundle: Bundle, filepath: string): Promise<number> {
  const lib = getLib();
  const ptr = await lib.symbols.wvb_write_bundle(bundle.pointer, cstr(filepath));
  return JSON.parse(readResult(lib, ptr).json) as number;
}

/** Serializes a bundle to `.wvb` bytes. */
export function writeBundleToBytes(bundle: Bundle): Uint8Array<ArrayBuffer> {
  const lib = getLib();
  const ptr = lib.symbols.wvb_write_bundle_to_bytes(bundle.pointer);
  return readResult(lib, ptr).body;
}
