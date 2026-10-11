import { defineConfig } from 'tsdown';

export default defineConfig([
  {
    entry: [
      './src/index.ts',
      './src/lambda/index.ts',
      './src/pulumi/index.ts',
      './src/cdk/index.ts',
    ],
    format: ['esm', 'cjs'],
    platform: 'node',
    target: 'node22',
    dts: true,
    clean: true,
    // Keep asset paths relative to each public entry point in both module formats.
    fixedExtension: true,
  },
  {
    entry: './assets/origin-request.ts',
    outDir: './dist/assets/',
    format: ['esm'],
    platform: 'node',
    target: 'node22',
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
      neverBundle: [/^@aws-sdk\//, /^node:/],
      onlyImport: ['@aws-sdk/client-s3'],
      onlyBundle: false,
    },
  },
]);
