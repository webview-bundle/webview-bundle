import { defineProject, type ViteUserConfig } from 'vitest/config';

const config: ViteUserConfig = defineProject({
  test: {
    name: 'wvb-tauri/e2e',
    include: ['**/*.spec.ts'],
    globalSetup: ['./global-setup.ts'],
    fileParallelism: false,
    pool: 'forks',
    testTimeout: 60_000,
    hookTimeout: 180_000,
    retry: 2,
  },
});

export { config as default };
