import { defineProject, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineProject({
  test: {
    clearMocks: true,
    environment: 'node',
    include: ['src/**/*.spec.ts'],
  },
});

export { config as default };
