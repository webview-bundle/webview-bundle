import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: ['./src/index.ts', './src/testing/index.ts'],
  format: ['esm', 'cjs'],
  platform: 'neutral',
  target: ['node18', 'es2020'],
  dts: true,
  clean: true,
  deps: {
    onlyBundle: false,
  },
});

export { config as default };
