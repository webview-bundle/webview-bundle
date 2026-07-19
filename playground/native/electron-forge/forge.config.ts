import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { MakerDMG } from '@electron-forge/maker-dmg';
import { MakerZIP } from '@electron-forge/maker-zip';
import { AutoUnpackNativesPlugin } from '@electron-forge/plugin-auto-unpack-natives';
import { VitePlugin } from '@electron-forge/plugin-vite';
import type { ForgeConfig } from '@electron-forge/shared-types';
import { WebviewBundlePlugin } from '@wvb/electron-forge';

const require = createRequire(__filename);

/**
 * The `@wvb/node` native addon directory, resolved exactly as `@wvb/electron` sees it. In this
 * monorepo the npm `@wvb/node` is nested under `@wvb/electron` (the repo-root `@wvb/node` is the
 * workspace copy), so resolving from `@wvb/electron` picks the right one.
 */
function wvbNodeDir(): string {
  const electronPkg = require.resolve('@wvb/electron/package.json');
  const nodePkg = createRequire(electronPkg).resolve('@wvb/node/package.json');
  return path.dirname(nodePkg);
}

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
    // In this monorepo `@wvb/node` is hoisted, so the packager never copies it out of the app dir.
    // Copy the resolved native addon into the packaged app after pruning so it ships (and
    // auto-unpack-natives can unpack its `.node`). A standalone consumer already has it in the
    // local node_modules (kept by the `ignore` above), making this a harmless refresh.
    afterPrune: [
      (
        buildPath: string,
        _version: string,
        _platform: string,
        _arch: string,
        done: (err?: Error) => void
      ) => {
        const dest = path.join(buildPath, 'node_modules', '@wvb', 'node');
        fs.rm(dest, { recursive: true, force: true })
          .then(() => fs.cp(wvbNodeDir(), dest, { recursive: true, dereference: true }))
          .then(() => done())
          .catch(done);
      },
    ],
  },
  // MakerDMG is macOS-only (it builds on `darwin`), so it is skipped on other platforms.
  // `name` is the DMG volume name; it must be <= 27 chars (a macOS alias limit), so it can't
  // default to the long package name.
  makers: [new MakerZIP({}), new MakerDMG({ name: 'WVB Forge' })],
  plugins: [
    new VitePlugin({
      build: [
        { entry: 'src/main.ts', config: 'vite.main.config.ts' },
        { entry: 'src/preload.ts', config: 'vite.preload.config.ts' },
      ],
      renderer: [],
    }),
    new AutoUnpackNativesPlugin({}),
    // `builtin: {}` is passed inline (rather than in wvb.config.ts) so the plugin installs builtin
    // bundles from the `remote.endpoint` resolved from wvb.config.ts at package time.
    new WebviewBundlePlugin({ builtin: {} }),
  ],
};

export default config;
