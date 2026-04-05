import { defineConfig, type UserConfig } from 'tsdown';

const config: UserConfig = defineConfig({
  // entry: ['./api/index.ts', './api/remote.ts', './api/source.ts', './api/updater.ts'],
  entry: {
    'api/index': './api/index.ts',
    'api/remote': './api/remote.ts',
    'api/source': './api/source.ts',
    'api/update': './api/updater.ts',
  },
  external: ['@tauri/*'],
  format: ['esm', 'cjs'],
  platform: 'node',
  target: 'es2020',
  dts: true,
  clean: true,
});

export { config as default };
