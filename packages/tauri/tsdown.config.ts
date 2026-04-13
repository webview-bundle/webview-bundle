import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  entry: {
    'api/index': './api/index.ts',
    'api/remote/index': './api/remote.ts',
    'api/source/index': './api/source.ts',
    'api/updater/index': './api/updater.ts',
  },
  external: ['@tauri/*'],
  format: ['esm', 'cjs'],
  platform: 'neutral',
  target: 'es2020',
  dts: true,
  clean: true,
});

export { config as default };
