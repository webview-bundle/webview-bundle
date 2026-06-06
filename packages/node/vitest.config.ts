import { defineProject, type UserWorkspaceConfig } from 'vitest/config';

const config: UserWorkspaceConfig = defineProject({
  test: {
    include: ['tests/**/*.{spec,test}.ts'],
    clearMocks: true,
    environment: 'node',
  },
});

export { config as default };
