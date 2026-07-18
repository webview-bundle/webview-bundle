import { defineConfig } from 'tsdown';

export default defineConfig({
  entry: ['./testing/index.ts'],
  outDir: 'dist-testing',
  format: 'esm',
  target: 'node24',
  platform: 'node',
  dts: true,
  clean: true,
  treeshake: true,
});
