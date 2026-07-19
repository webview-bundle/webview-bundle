import { MakerZIP } from '@electron-forge/maker-zip';
import { AutoUnpackNativesPlugin } from '@electron-forge/plugin-auto-unpack-natives';
import { VitePlugin } from '@electron-forge/plugin-vite';
import type { ForgeConfig } from '@electron-forge/shared-types';
import { WebviewBundlePlugin } from '@wvb/electron-forge';

const config: ForgeConfig = {
  packagerConfig: {
    asar: true,
    // The Forge Vite plugin's default ignore keeps ONLY `.vite`, which drops every node_modules —
    // including the native `@wvb/node` addon that Vite cannot bundle. Setting our own `ignore`
    // overrides that default: keep the Vite output, the app manifest, and the `@wvb` scope (so the
    // `@wvb/node` addon ships and auto-unpack-natives can unpack its `.node` out of the asar).
    ignore: (file: string): boolean => {
      if (!file) return false;
      if (file.startsWith('/.vite') || file === '/package.json') return false;
      if (
        file === '/node_modules' ||
        file === '/node_modules/@wvb' ||
        file.startsWith('/node_modules/@wvb/')
      ) {
        return false;
      }
      return true;
    },
  },
  makers: [new MakerZIP({})],
  plugins: [
    new VitePlugin({
      build: [
        { entry: 'src/main.ts', config: 'vite.main.config.ts' },
        { entry: 'src/preload.ts', config: 'vite.preload.config.ts' },
      ],
      renderer: [],
    }),
    new AutoUnpackNativesPlugin({}),
    new WebviewBundlePlugin(),
  ],
};

export default config;
