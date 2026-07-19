import fs from 'node:fs/promises';
import path from 'node:path';

// The `binding.cjs` that `@napi-rs/cli` generates has a bug in its
// `NAPI_RS_NATIVE_LIBRARY_PATH` branch: unlike every other resolution branch it
// assigns the loaded binding to the module-level `nativeBinding` instead of
// returning it, so the subsequent `nativeBinding = requireNative()` immediately
// overwrites it with `undefined` and loading fails with "Failed to load native
// binding". `@wvb/electron` relies on this env var to load the arch-specific binary
// it bundles, so we re-apply this fix after every `napi build` (and keep the
// committed copy fixed for consumers who install without building).
const bindingPath = path.join(import.meta.dirname, '..', 'binding.cjs');

const BUGGY = '      nativeBinding = require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH);';
const FIXED = '      return require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH);';

const source = await fs.readFile(bindingPath, 'utf8');

if (source.includes(FIXED)) {
  console.log('[patch-binding] already patched');
} else if (source.includes(BUGGY)) {
  await fs.writeFile(bindingPath, source.replace(BUGGY, FIXED), 'utf8');
  console.log('[patch-binding] patched NAPI_RS_NATIVE_LIBRARY_PATH branch');
} else {
  console.error(
    '[patch-binding] could not find the NAPI_RS_NATIVE_LIBRARY_PATH branch to patch. ' +
      'The @napi-rs/cli codegen likely changed; re-check whether the env-var branch still ' +
      'needs the return fix and update this script.'
  );
  process.exit(1);
}
