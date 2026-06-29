import denoJson from '../deno.json' with { type: 'json' };

// Shared logic for locating, downloading, and verifying the `wvb-deno` release cdylib.
//
// The release pipeline builds the cdylib per platform and uploads it to the GitHub release tagged
// `deno/<version>` as `<prefix>wvb_deno-<target>.<ext>` (e.g.
// `libwvb_deno-aarch64-apple-darwin.dylib`), each alongside a `<asset>.sha256` integrity sidecar
// (`sha256sum` format) written by `scripts/build.ts`. These helpers reproduce that asset naming and
// verify a downloaded file against its sidecar. Used by both `install.ts` (download-to-vendor) and
// `ffi.ts` (`loadLibViaPlug`, runtime download).

/** Base URL of the repo's GitHub release downloads. */
export const DEFAULT_RELEASE_BASE =
  'https://github.com/webview-bundle/webview-bundle/releases/download';

/**
 * This package's version, read from `deno.json`. Used as the default release tag suffix
 * (`deno/<VERSION>`) so `install()` / `loadLibViaPlug()` resolve the cdylib that matches the
 * installed `@wvb/deno` — `prepare-release` bumps `deno.json` before publishing, so a published
 * package always points at its own release.
 */
export const VERSION: string = denoJson.version;

/**
 * Rust target triples we publish a `wvb-deno` cdylib for. Must stay in sync with the `build-deno`
 * matrix in `.github/workflows/release.yaml`.
 */
export const SUPPORTED_TARGETS = [
  'aarch64-apple-darwin',
  'x86_64-apple-darwin',
  'aarch64-unknown-linux-gnu',
  'x86_64-unknown-linux-gnu',
  'x86_64-pc-windows-msvc',
] as const;

export type Os = typeof Deno.build.os;

export function osOfTarget(target: string): Os {
  if (target.includes('darwin') || target.includes('apple')) {
    return 'darwin';
  }
  if (target.includes('windows') || target.includes('msvc')) {
    return 'windows';
  }
  return 'linux';
}

function ext(os: Os): string {
  return os === 'windows' ? 'dll' : os === 'darwin' ? 'dylib' : 'so';
}

function prefix(os: Os): string {
  return os === 'windows' ? '' : 'lib';
}

/**
 * Plain platform filename cargo emits and we save locally for a single-target build, e.g.
 * `libwvb_deno.dylib` (macOS) / `libwvb_deno.so` (Linux) / `wvb_deno.dll` (Windows).
 */
export function localFileName(target: string): string {
  const os = osOfTarget(target);
  return `${prefix(os)}wvb_deno.${ext(os)}`;
}

/** Release asset name for a target, e.g. `libwvb_deno-aarch64-apple-darwin.dylib`. */
export function releaseAssetName(target: string): string {
  const os = osOfTarget(target);
  return `${prefix(os)}wvb_deno-${target}.${ext(os)}`;
}

/** Default release base for a version: `<repo>/releases/download/deno/<version>` (no trailing slash). */
export function releaseBaseUrl(version: string): string {
  return `${DEFAULT_RELEASE_BASE}/deno/${version}`;
}

type CrossRecord = Record<string, Record<string, string>>;

/**
 * `@denosaurs/plug` `suffixes` map so a `name: 'wvb_deno'` download resolves to our
 * `<prefix>wvb_deno-<target>.<ext>` asset names. plug applies the per-os prefix (`lib`) and
 * extension (`.dylib`/`.so`/`.dll`) itself; we only supply the `-<target>` middle part.
 */
export function releaseAssetSuffixes(): CrossRecord {
  const map: CrossRecord = {};
  for (const target of SUPPORTED_TARGETS) {
    const os = osOfTarget(target);
    // First triple segment is the arch (`aarch64` / `x86_64`) — matches plug's arch keys.
    const arch = target.split('-')[0]!;
    map[os] ??= {};
    map[os][arch] = `-${target}`;
  }
  return map;
}

/** Lowercase hex SHA-256 of `bytes`. */
export async function sha256Hex(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes as BufferSource);
  return Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2, '0')).join('');
}

/** Parse `sha256sum`-format text (`<hex>  [*]<filename>` per line) into a filename → hash map. */
export function parseChecksums(text: string): Map<string, string> {
  const map = new Map<string, string>();
  for (const line of text.split('\n')) {
    // `sha256sum` separates hash and name with two spaces; the `*` marks binary mode.
    const match = line.trim().match(/^([0-9a-fA-F]{64})\s+\*?(.+)$/);
    if (match != null) {
      map.set(match[2]!.trim(), match[1]!.toLowerCase());
    }
  }
  return map;
}

/**
 * Fetch the `<base>/<assetName>.sha256` integrity sidecar (written by `scripts/build.ts`) and return
 * the expected SHA-256 for `assetName`. Throws when the sidecar can't be fetched or doesn't list the
 * asset (fail closed — never silently skip verification).
 */
export async function fetchExpectedChecksum(base: string, assetName: string): Promise<string> {
  const url = `${base.replace(/\/+$/, '')}/${assetName}.sha256`;
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(
      `wvb: failed to download ${url} (${res.status} ${res.statusText}). ` +
        'Pass `integrity: false` to skip checksum verification.'
    );
  }
  const checksums = parseChecksums(await res.text());
  // The sidecar lists exactly one entry; accept it under the asset name, or as the sole line.
  const expected =
    checksums.get(assetName) ?? (checksums.size === 1 ? [...checksums.values()][0] : undefined);
  if (expected == null) {
    throw new Error(`wvb: ${assetName} not listed in ${url}`);
  }
  return expected;
}

/**
 * Verify `bytes` against the expected SHA-256 for `assetName` from its `.sha256` sidecar. Throws on
 * any mismatch or when the checksum can't be resolved.
 */
export async function verifyChecksum(
  bytes: Uint8Array,
  base: string,
  assetName: string
): Promise<void> {
  const expected = await fetchExpectedChecksum(base, assetName);
  const actual = await sha256Hex(bytes);
  if (actual !== expected) {
    throw new Error(
      `wvb: checksum mismatch for ${assetName}\n  expected ${expected}\n  actual   ${actual}`
    );
  }
}
