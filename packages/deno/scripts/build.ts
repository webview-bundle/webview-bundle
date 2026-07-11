// Build the `wvb-deno` cdylib for a target and stage it under `.output/` with its release asset
// name + a `<asset>.sha256` sidecar, ready for `xtask artifacts merge` to collect and `xtask
// release` to upload. The sidecar (sha256sum format) is what the `@wvb/deno` client verifies the
// download against (see lib/release.ts) — computing it here keeps the integrity hash in the package
// rather than in the release workflow, and each platform build owns its own asset's checksum.
//
//   deno run --allow-read --allow-write --allow-run --allow-env scripts/build.ts [--target <triple>] [--zigbuild]
//
// `--target` defaults to the host (`Deno.build.target`). Pass `--zigbuild` to cross-compile via
// `cargo zigbuild` (used for the aarch64 Linux target in CI); otherwise plain `cargo build` is used.
import { fromFileUrl } from '@std/path';
import { libFileName } from '../lib/ffi.ts';
import { osOfTarget, releaseAssetName, sha256Hex } from '../lib/release.ts';

const PKG_DIR = fromFileUrl(new URL('../', import.meta.url));
const ROOT_DIR = fromFileUrl(new URL('../../../', import.meta.url));

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
  const subcommand = Deno.args.includes('--zigbuild') ? 'zigbuild' : 'build';

  console.log(`wvb: cargo ${subcommand} -p wvb-deno --release --target ${target}`);
  const build = new Deno.Command('cargo', {
    args: [subcommand, '-p', 'wvb-deno', '--release', '--target', target],
    cwd: ROOT_DIR,
    stdout: 'inherit',
    stderr: 'inherit',
  });
  const { code } = await build.output();
  if (code !== 0) {
    Deno.exit(code);
  }

  const assetName = releaseAssetName(target);
  const src = `${ROOT_DIR}target/${target}/release/${libFileName(osOfTarget(target))}`;
  const outDir = `${PKG_DIR}.output`;
  const dest = `${outDir}/${assetName}`;
  await Deno.mkdir(outDir, { recursive: true });
  await Deno.copyFile(src, dest);
  console.log(`wvb: staged ${dest}`);

  // Write the `sha256sum`-format integrity sidecar next to the asset.
  const hex = await sha256Hex(await Deno.readFile(dest));
  await Deno.writeTextFile(`${dest}.sha256`, `${hex}  ${assetName}\n`);
  console.log(`wvb: wrote ${dest}.sha256 (${hex})`);
}

if (import.meta.main) {
  await main();
}
