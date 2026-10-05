import { defineConfig, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineConfig({
  test: {
    projects: ['packages/*/vitest.config.ts', 'packages/remote/*/vitest.config.ts'],
    testTimeout: 10_000,
  },
});

export default config;
