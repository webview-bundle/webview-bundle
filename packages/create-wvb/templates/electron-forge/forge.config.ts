import { MakerDeb } from '@electron-forge/maker-deb';
import { MakerSquirrel } from '@electron-forge/maker-squirrel';
import { MakerZIP } from '@electron-forge/maker-zip';
import { AutoUnpackNativesPlugin } from '@electron-forge/plugin-auto-unpack-natives';
import type { ForgeConfig } from '@electron-forge/shared-types';
import { WebviewBundlePlugin } from '@wvb/electron-forge';

const config: ForgeConfig = {
  packagerConfig: {
    asar: true,
    // Matched against paths relative to the project root, each with a leading "/".
    // "bundles" is excluded on purpose: WebviewBundlePlugin copies it into the packaged
    // app's "resources/bundles", next to the asar — keeping it here too would pack a
    // second, unreachable copy of every .wvb inside the asar.
    ignore: [
      /^\/\.wvb($|\/)/,
      /^\/bundles($|\/)/,
      /^\/web($|\/)/,
      /^\/forge\.config\.ts$/,
      /^\/wvb\.config\.ts$/,
      /^\/README\.md$/,
    ],
  },
  makers: [new MakerSquirrel({}), new MakerZIP({}, ['darwin']), new MakerDeb({})],
  plugins: [
    // Unpacks the native @wvb/node addon (**/*.node) out of the asar so it can be loaded.
    new AutoUnpackNativesPlugin({}),
    // Reads builtin/remote settings from wvb.config.ts, installs the .wvb bundles, and copies
    // them into the packaged app's "resources/bundles" — where @wvb/electron reads them.
    // This runs at package time only: `electron-forge start` never installs bundles.
    new WebviewBundlePlugin({}),
  ],
};

export default config;
