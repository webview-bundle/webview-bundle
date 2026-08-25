import { defineConfig } from 'tsdown';

export default defineConfig({
  entry: { main: './src/main.ts', preload: './src/preload.ts' },
  outDir: './dist',
  format: 'cjs',
  platform: 'node',
  target: 'node20',
  dts: false,
  clean: true,
  deps: {
    /**
     * `@wvb/electron` stays external in main so its native `@wvb/node` addon is loaded from
     * node_modules rather than inlined. The preload is the opposite: it must be self-contained,
     * because a sandboxed preload can only `require('electron')` — see README.
     */
    neverBundle: ['electron'],
    alwaysBundle: ['@wvb/electron/preload'],
  },
});
