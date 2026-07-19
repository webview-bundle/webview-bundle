import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  // `native.ts` is its own entry so it stays a separate module that runs before the
  // `@wvb/node` import (see `native.ts`); inlining it into `index` would let the
  // hoisted `@wvb/node` import load the binding first.
  entry: ['./src/index.ts', './src/native.ts', './src/preload/index.ts'],
  format: ['esm', 'cjs'],
  platform: 'node',
  target: 'node12',
  dts: true,
  clean: true,
  // Provide `import.meta.url` in the CJS build and `__dirname` in the ESM build so
  // `native.ts` can resolve the bundled binary directory in both formats.
  shims: true,
});

export { config as default };
