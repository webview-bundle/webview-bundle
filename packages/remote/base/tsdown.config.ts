import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: ['./src/index.ts', './src/server/index.ts'],
  format: ['esm', 'cjs'],
  platform: 'node',
  target: 'node22',
  deps: {
    onlyBundle: false,
  },
  dts: true,
  clean: true,
});

export { config as default };
