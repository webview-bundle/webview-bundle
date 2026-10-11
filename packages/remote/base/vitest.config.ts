import { defineProject, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineProject({
  test: {
    clearMocks: true,
    environment: 'node',
  },
});

export { config as default };
