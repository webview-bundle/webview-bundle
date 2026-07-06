import { defineConfig } from './xtask/config.ts';

export default defineConfig({
  // A directory becomes a package when it directly holds a versioned manifest
  // (`package.json`/`Cargo.toml`/`deno.json`); everything below it (e.g. napi platform
  // packages under `packages/node/npm/*`) is versioned as part of that package.
  packages: [
    'packages/*',
    'packages/remote/*',
    {
      path: 'packages/node',
      beforePublishScripts: [{ command: 'yarn', args: ['artifacts'] }],
      artifacts: [{ src: '.', patterns: ['*.node'], dest: 'artifacts' }],
      assets: ['artifacts/*.node'],
    },
    {
      path: 'packages/ffi',
      artifacts: [
        {
          src: '.output',
          patterns: ['apple.zip', 'WebViewBundleFFI.xcframework.zip', 'android.zip'],
          dest: '.output',
        },
      ],
      assets: [
        '.output/android.zip',
        '.output/apple.zip',
        '.output/WebViewBundleFFI.xcframework.zip',
      ],
    },
    {
      path: 'packages/deno',
      artifacts: [
        {
          src: '.output',
          patterns: ['libwvb_deno-*.dylib', 'libwvb_deno-*.so', 'wvb_deno-*.dll', '*.sha256'],
          dest: '.output',
        },
      ],
      assets: [
        '.output/libwvb_deno-*.dylib',
        '.output/libwvb_deno-*.so',
        '.output/wvb_deno-*.dll',
        '.output/*.sha256',
      ],
    },
  ],
});
