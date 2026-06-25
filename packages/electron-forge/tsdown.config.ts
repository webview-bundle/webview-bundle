import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: ['./src/index.ts'],
  format: ['esm', 'cjs'],
  platform: 'node',
  target: 'node18',
  dts: true,
  clean: true,
});

export { config as default };
