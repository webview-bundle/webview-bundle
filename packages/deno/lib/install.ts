// CLI: download the wvb-deno cdylib from GitHub Releases into your project so you can `--include` it
// in a `deno desktop` / `deno compile` build.
//
//   deno run -A jsr:@wvb/deno/install [--out vendor/wvb] [--target <triple>] [--version <v>] [--url <base>]
//
// The release asset is named `<prefix>wvb_deno-<target>.<ext>` (e.g. `libwvb_deno-aarch64-apple-darwin.dylib`)
// and is saved locally under the plain platform name (`libwvb_deno.dylib`), since a single compiled
// binary targets one platform. Then: `deno desktop --allow-ffi --include <out>/<file> main.ts`.

const DEFAULT_RELEASE_BASE = 'https://github.com/webview-bundle/webview-bundle/releases/download';

type Os = typeof Deno.build.os;

function osOfTarget(target: string): Os {
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

/** Plain platform filename to save locally (single-target build), e.g. `libwvb_deno.dylib`. */
export function localFileName(target: string): string {
  const os = osOfTarget(target);
  return `${prefix(os)}wvb_deno.${ext(os)}`;
}

/** Release asset name for a target, e.g. `libwvb_deno-aarch64-apple-darwin.dylib`. */
export function releaseAssetName(target: string): string {
  const os = osOfTarget(target);
  return `${prefix(os)}wvb_deno-${target}.${ext(os)}`;
}

export interface InstallOptions {
  target?: string;
  out?: string;
  version?: string;
  /** Release base URL; defaults to `<repo>/releases/download/deno@<version>`. */
  url?: string;
}

/** Download the cdylib for `target` into `out`, returning the saved file path. */
export async function install(options: InstallOptions = {}): Promise<string> {
  const target = options.target ?? Deno.build.target;
  const out = options.out ?? 'vendor/wvb';
  const version = options.version ?? '0.0.0';
  const base = options.url ?? `${DEFAULT_RELEASE_BASE}/deno@${version}`;
  const url = `${base}/${releaseAssetName(target)}`;

  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`wvb: failed to download ${url} (${res.status} ${res.statusText})`);
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  await Deno.mkdir(out, { recursive: true });
  const dest = `${out}/${localFileName(target)}`;
  await Deno.writeFile(dest, bytes);
  return dest;
}

function flag(name: string): string | undefined {
  const i = Deno.args.indexOf(`--${name}`);
  if (i < 0) {
    return undefined;
  }
  // Don't consume the next token if it's another flag (i.e. this flag's value is missing).
  const value = Deno.args[i + 1];
  return value != null && !value.startsWith('--') ? value : undefined;
}

async function main(): Promise<void> {
  const target = flag('target') ?? Deno.build.target;
  const out = flag('out') ?? 'vendor/wvb';
  console.log(`wvb: downloading cdylib for ${target}…`);
  const dest = await install({
    target,
    out,
    version: flag('version'),
    url: flag('url'),
  });
  console.log(`wvb: saved ${dest}`);
  console.log(`Build with: deno desktop --allow-ffi --include ${dest} main.ts`);
}

if (import.meta.main) {
  await main();
}
