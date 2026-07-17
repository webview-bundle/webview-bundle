import { cstr, getLib, readResult } from './ffi.ts';

/** Hash algorithm for a bundle integrity digest. */
export type IntegrityAlgorithm = 'sha256' | 'sha384' | 'sha512';

/**
 * A digest over some bytes, serialized as `<algorithm>:<base64>` (e.g. `"sha256:n4bQ..."`).
 *
 * Created by {@linkcode computeIntegrity} or {@linkcode parseIntegrity}. Unlike the other
 * classes here it owns no native handle — the digest is copied out on creation — so it needs
 * no `free()`.
 */
export class Integrity {
  readonly #serialized: string;
  readonly #value: Uint8Array<ArrayBuffer>;

  /** @internal Use {@linkcode computeIntegrity} or {@linkcode parseIntegrity}. */
  constructor(serialized: string, value: Uint8Array<ArrayBuffer>) {
    this.#serialized = serialized;
    this.#value = value;
  }

  /** The raw digest bytes. */
  value(): Uint8Array<ArrayBuffer> {
    return this.#value;
  }

  /** Whether `data` digests to this integrity. */
  validate(data: Uint8Array<ArrayBuffer>): boolean {
    const lib = getLib();
    const ptr = lib.symbols.wvb_integrity_validate(
      cstr(this.#serialized),
      data,
      BigInt(data.byteLength)
    );
    return JSON.parse(readResult(lib, ptr).json) as boolean;
  }

  /** Serializes to `<algorithm>:<base64>`. */
  serialize(): string {
    return this.#serialized;
  }

  toString(): string {
    return this.#serialized;
  }
}

/**
 * Computes the integrity of `data` with `algorithm`.
 *
 * This is the write side of integrity: use it when publishing a bundle to produce the string
 * a source or updater later verifies against.
 *
 * @example
 * ```ts
 * const integrity = computeIntegrity('sha256', data).serialize();
 * // "sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg="
 * ```
 */
export function computeIntegrity(
  algorithm: IntegrityAlgorithm,
  data: Uint8Array<ArrayBuffer>
): Integrity {
  const lib = getLib();
  const ptr = lib.symbols.wvb_compute_integrity(cstr(algorithm), data, BigInt(data.byteLength));
  const result = readResult(lib, ptr);
  return new Integrity(JSON.parse(result.json) as string, result.body);
}

/**
 * Parses a serialized integrity string (e.g. `"sha256:n4bQ..."`).
 *
 * @example
 * ```ts
 * const isValid = parseIntegrity(advertised).validate(data);
 * ```
 */
export function parseIntegrity(integrity: string): Integrity {
  const lib = getLib();
  const ptr = lib.symbols.wvb_parse_integrity(cstr(integrity));
  const result = readResult(lib, ptr);
  return new Integrity(JSON.parse(result.json) as string, result.body);
}
