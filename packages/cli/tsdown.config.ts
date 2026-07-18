import { defineConfig, type UserConfig } from 'tsdown';

const shared = {
  platform: 'node',
  target: 'node18',
  clean: false,
} satisfies Partial<UserConfig>;

const config: UserConfig[] = defineConfig([
  {
    ...shared,
    entry: ['./src/index.ts', './src/api/index.ts'],
    format: ['esm', 'cjs'],
    dts: true,
  },
  {
    ...shared,
    entry: ['./src/cli.ts'],
    format: ['esm'],
    dts: false,
  },
]);

export { config as default };
