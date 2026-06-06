import { defineConfig, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineConfig({
  test: {
    projects: ['packages/*/e2e/vitest.e2e.config.ts'],
  },
});

export default config;
