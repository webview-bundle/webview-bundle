import { defineProject, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineProject({
  test: {
    clearMocks: true,
    environment: 'node',
    testTimeout: 10_000,
  },
});

export { config as default };
