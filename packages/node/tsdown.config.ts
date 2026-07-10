import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: ['./lib/index.ts'],
  outDir: './dist',
  clean: true,
  format: ['esm', 'cjs'],
  platform: 'node',
  target: 'node18',
  dts: true,
  outExtensions: ({ format }) =>
    format === 'es' ? { js: '.js', dts: '.d.ts' } : { js: '.cjs', dts: '.d.cts' },
  deps: {
    // `#binding` is a subpath import resolved per-format (binding.js / binding.cjs) by Node itself.
    neverBundle: [/^#/],
  },
});

export { config as default };
