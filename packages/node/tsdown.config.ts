import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: ['./lib/index.ts', './lib/binding/index.ts'],
  outDir: './dist',
  clean: true,
  format: ['esm', 'cjs'],
  platform: 'node',
  target: 'node18',
  dts: true,
  shims: true,
  outExtensions: ({ format }) =>
    format === 'es' ? { js: '.js', dts: '.d.ts' } : { js: '.cjs', dts: '.d.cts' },
  deps: {
    neverBundle: [/binding\.cjs$/],
  },
});

export { config as default };
