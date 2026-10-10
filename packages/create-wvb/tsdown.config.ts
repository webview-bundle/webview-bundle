import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: ['./src/cli.ts'],
  format: ['esm'],
  platform: 'node',
  target: 'node18',
  dts: false,
  clean: false,
  minify: true,
});

export { config as default };
