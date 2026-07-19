import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';

// Thin wrapper around `napi build` that forwards any extra CLI args (e.g.
// `--target <triple>`, `-x`, `--use-napi-cross`) to napi and then re-applies the
// `binding.cjs` fix (see `patch-binding.ts`). Chaining `napi build && node
// patch-binding.ts` in the npm script would not work: `yarn run build-napi --target
// x` appends `--target x` to the *end* of the command, so it would reach
// patch-binding instead of napi and the target would be silently ignored.
const require = createRequire(import.meta.url);
const napiPkg = require.resolve('@napi-rs/cli/package.json');
const napiBin = path.join(path.dirname(napiPkg), (require(napiPkg).bin as { napi: string }).napi);
const pkgDir = path.join(import.meta.dirname, '..');

const build = spawnSync(
  process.execPath,
  [
    napiBin,
    'build',
    '--platform',
    '--release',
    '--js=binding.cjs',
    '--dts=binding.d.cts',
    '--no-const-enum',
    '--no-dts-cache',
    ...process.argv.slice(2),
  ],
  { stdio: 'inherit', cwd: pkgDir }
);
if (build.status !== 0) {
  process.exit(build.status ?? 1);
}

const patch = spawnSync(process.execPath, [path.join(import.meta.dirname, 'patch-binding.ts')], {
  stdio: 'inherit',
});
if (patch.status !== 0) {
  process.exit(patch.status ?? 1);
}
