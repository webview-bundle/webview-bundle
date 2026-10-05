import { defineConfig, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineConfig({
  test: {
    projects: ['packages/*/vitest.config.ts', 'packages/remote/*/vitest.config.ts'],
  },
});

export default config;
