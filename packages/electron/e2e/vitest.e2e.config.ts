import { defineProject, type ViteUserConfig } from 'vitest/config';

const config: ViteUserConfig = defineProject({
  test: {
    name: '@wvb/electron/e2e',
    include: ['**/*.spec.ts'],
    fileParallelism: false,
    pool: 'forks',
    testTimeout: 60_000,
    hookTimeout: 60_000,
  },
});

export { config as default };
