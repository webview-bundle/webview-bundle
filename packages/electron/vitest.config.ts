import { defineProject, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineProject({
  test: {
    include: ['src/**/*.spec.ts'],
    clearMocks: true,
    environment: 'node',
  },
});

export { config as default };
