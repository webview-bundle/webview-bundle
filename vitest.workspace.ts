import { defineConfig, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineConfig({
  test: {
    projects: ['packages/*/vitest.config.ts', 'xtask/vitest.config.ts'],
  },
});

export default config;
