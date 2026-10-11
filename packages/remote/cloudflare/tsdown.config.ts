import { defineConfig } from 'tsdown';

export default defineConfig([
  {
    entry: ['./src/index.ts', './src/worker/index.ts', './src/pulumi/index.ts'],
    format: ['esm', 'cjs'],
    platform: 'node',
    target: 'node22',
    dts: true,
    clean: true,
    // Keep asset paths relative to each public entry point in both module formats.
    fixedExtension: true,
  },
  {
    entry: './assets/worker.ts',
    outDir: './dist/assets/',
    format: ['esm'],
    platform: 'neutral',
    target: 'es2022',
    minify: false,
    dts: false,
    fixedExtension: true,
    inputOptions: {
      experimental: {
        attachDebugInfo: 'none',
      },
    },
    deps: {
      alwaysBundle: [/.*/],
      neverBundle: [/^node:/],
      onlyBundle: false,
    },
  },
]);
