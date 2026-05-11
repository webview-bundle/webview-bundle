import { FuseV1Options, FuseVersion } from '@electron/fuses';
import { MakerDMG } from '@electron-forge/maker-dmg';
import { AutoUnpackNativesPlugin } from '@electron-forge/plugin-auto-unpack-natives';
import { FusesPlugin } from '@electron-forge/plugin-fuses';
import { VitePlugin } from '@electron-forge/plugin-vite';
import type { ForgeConfig } from '@electron-forge/shared-types';

const config: ForgeConfig = {
  packagerConfig: {
    asar: true,
    extraResource: ['bundles'],
    overwrite: true,
    // Electron forge vite plugin default configuration behaves ignores all files except files in '.vite' directory.
    // So the native modules from 'node_modules/' cannot be unpacked because it has been ignored by default
    // configuration.
    // https://github.com/electron/forge/issues/3934
    // https://github.com/electron/forge/blob/v7.10.2/packages/plugin/vite/src/VitePlugin.ts#L178
    ignore: (file: string) => {
      if (!file) return false;
      const keep =
        file.startsWith('/.vite') || file.startsWith('/node_modules') || file.endsWith('.tgz');
      return !keep;
    },
  },
  rebuildConfig: {},
  makers: [
    new MakerDMG({
      // This should be specified
      // @see https://github.com/LinusU/node-appdmg/issues/48
      title: 'electron-forge-vite',
    }),
  ],
  plugins: [
    new VitePlugin({
      // `build` can specify multiple entry builds, which can be Main process, Preload scripts, Worker process, etc.
      // If you are familiar with Vite configuration, it will look really familiar.
      build: [
        {
          // `entry` is just an alias for `build.lib.entry` in the corresponding file of `config`.
          entry: 'src/main.ts',
          config: 'vite.main.config.ts',
          target: 'main',
        },
        {
          entry: 'src/preload.ts',
          config: 'vite.preload.config.ts',
          target: 'preload',
        },
      ],
      renderer: [
        {
          name: 'main_window',
          config: 'vite.renderer.config.ts',
        },
      ],
    }),
    // Fuses are used to enable/disable various Electron functionality
    // at package time, before code signing the application
    new FusesPlugin({
      version: FuseVersion.V1,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true,
    }),
    new AutoUnpackNativesPlugin({}),
  ],
};

export default config;
