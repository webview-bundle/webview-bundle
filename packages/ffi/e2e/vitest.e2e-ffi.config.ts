import { defineConfig, type ViteUserConfig } from 'vitest/config';

const config: ViteUserConfig = defineConfig({
  test: {
    name: '@wvb/ffi/e2e',
    include: ['**/*.spec.ts'],
    fileParallelism: false,
    pool: 'forks',
    testTimeout: 30_000,
    hookTimeout: 1_800_000,
  },
});

export { config as default };
