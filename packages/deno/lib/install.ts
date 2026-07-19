// CLI: download the wvb-deno cdylib from GitHub Releases into your project so you can `--include` it
// in a `deno desktop` / `deno compile` build.
//
//   deno run -A jsr:@wvb/deno/install [--out .wvb/lib] [--target <triple>] [--version <v>] [--url <base>] [--no-integrity]
//
// The release asset is named `<prefix>wvb_deno-<target>.<ext>` (e.g. `libwvb_deno-aarch64-apple-darwin.dylib`)
// and is saved locally under the plain platform name (`libwvb_deno.dylib`), since a single compiled
// binary targets one platform. Then: `deno desktop --allow-ffi --include <out>/<file> main.ts`.
//
// Downloaded bytes are verified against the asset's `.sha256` sidecar before being written, unless
// `--no-integrity` is passed.

import { retry } from '@std/async';
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
   * @default .wvb/lib
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

class HttpError extends Error {
  override readonly name = 'HttpError';
  readonly status: number;

  static async from(res: Response): Promise<HttpError> {
    const message = await res.text().catch(() => undefined);
    return new HttpError(res.status, message);
  }

  constructor(status: number, message?: string) {
    super(message);
    this.status = status;
  }
}

export async function install(options: InstallOptions = {}): Promise<string> {
  const target = options.target ?? Deno.build.target;
  const out = options.out ?? '.wvb/lib';
  const version = options.version ?? VERSION;
  const base = options.url ?? releaseBaseUrl(version);
  const assetName = releaseAssetName(target);
  const url = `${base.replace(/\/+$/, '')}/${assetName}`;

  const res = await retry(
    async () => {
      const res = await fetch(url);
      if (!res.ok) {
        throw await HttpError.from(res);
      }
      return res;
    },
    {
      isRetriable: e => {
        if (e instanceof TypeError) {
          return true;
        }
        if (e instanceof HttpError) {
          return [429, 500, 503, 504].includes(e.status);
        }
        return false;
      },
    }
  );

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

  const value = Deno.args[i + 1];
  return value != null && !value.startsWith('--') ? value : undefined;
}

async function main(): Promise<void> {
  const target = flag('target') ?? Deno.build.target;
  const integrity = !Deno.args.includes('--no-integrity');
  console.log(
    `wvb: downloading cdylib for ${target}${integrity ? '' : ' (integrity check disabled)'}…`
  );
  const dest = await install({
    target,
    out: flag('out'),
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
