// CLI: download the wvb-deno cdylib from GitHub Releases into your project so you can `--include` it
// in a `deno desktop` / `deno compile` build.
//
//   deno run -A jsr:@wvb/deno/install [--out vendor/wvb] [--target <triple>] [--version <v>] [--url <base>] [--no-integrity]
//
// The release asset is named `<prefix>wvb_deno-<target>.<ext>` (e.g. `libwvb_deno-aarch64-apple-darwin.dylib`)
// and is saved locally under the plain platform name (`libwvb_deno.dylib`), since a single compiled
// binary targets one platform. Then: `deno desktop --allow-ffi --include <out>/<file> main.ts`.
//
// Downloaded bytes are verified against the asset's `.sha256` sidecar before being written, unless
// `--no-integrity` is passed.

import { libFileName } from './ffi.ts';
import {
  osOfTarget,
  releaseAssetName,
  releaseBaseUrl,
  VERSION,
  verifyChecksum,
} from './release.ts';

export interface InstallOptions {
  /**
   * Build target triple, which is the combination of `${arch}-${vendor}-${os}`.
   * @default Deno.build.target
   */
  target?: string;
  /**
   * Output directory.
   * @default vendor/wvb
   */
  out?: string;
  /**
   * Specify version to use.
   */
  version?: string;
  /**
   * Release base URL
   * @default https://github.com/webview-bundle/webview-bundle/releases/download/deno/<version>
   */
  url?: string;
  /**
   * Verify integrity with checksum
   * @default true
   */
  integrity?: boolean;
}

export async function install(options: InstallOptions = {}): Promise<string> {
  const target = options.target ?? Deno.build.target;
  const out = options.out ?? 'vendor/wvb';
  const version = options.version ?? VERSION;
  const base = options.url ?? releaseBaseUrl(version);
  const assetName = releaseAssetName(target);
  const url = `${base.replace(/\/+$/, '')}/${assetName}`;

  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`wvb: failed to download ${url} (${res.status} ${res.statusText})`);
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  if (options.integrity !== false) {
    await verifyChecksum(bytes, base, assetName);
  }
  await Deno.mkdir(out, { recursive: true });
  const dest = `${out}/${libFileName(osOfTarget(target))}`;
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
  const integrity = !Deno.args.includes('--no-integrity');
  console.log(
    `wvb: downloading cdylib for ${target}${integrity ? '' : ' (integrity check disabled)'}…`
  );
  const dest = await install({
    target,
    out,
    version: flag('version'),
    url: flag('url'),
    integrity,
  });
  console.log(`wvb: saved ${dest}`);
  console.log(`Build with: deno desktop --allow-ffi --include ${dest} main.ts`);
}

if (import.meta.main) {
  await main();
}
