import { defineConfig, type ViteUserConfig } from 'vitest/config';

const config: ViteUserConfig = defineConfig({
  test: {
    name: 'wvb-ffi/e2e',
    include: ['**/*.spec.ts'],
    fileParallelism: false,
    pool: 'forks',
    // Must exceed the specs' in-test wait budgets (apple.spec.ts waits up to 120s for the run
    // button + 180s for the suite to finish), or vitest kills the test before those waits get to
    // time out with their own, more specific error messages.
    testTimeout: 360_000,
    hookTimeout: 1_800_000,
  },
});

export { config as default };
