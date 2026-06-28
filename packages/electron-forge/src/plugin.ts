import fs from 'node:fs/promises';
import path from 'node:path';
import { PluginBase } from '@electron-forge/plugin-base';
import type {
  ForgeArch,
  ForgeMultiHookMap,
  ForgePlatform,
  ResolvedForgeConfig,
} from '@electron-forge/shared-types';
import { resolveConfig } from '@wvb/cli';
import { builtin } from '@wvb/cli/api';
import type { BuiltinTarget } from '@wvb/config';
import type { WebviewBundlePluginConfig } from './config.js';

const DEFAULT_BUILTIN_OUT_DIR = path.join('.wvb', 'builtin', 'bundles');
const DEFAULT_BUNDLES_DIR = 'bundles';

function safeBundlesDir(bundlesDir: string): string {
  if (path.isAbsolute(bundlesDir) || bundlesDir.split(/[/\\]/).includes('..')) {
    throw new Error(
      `bundlesDir must be a relative path without ".." segments (got "${bundlesDir}").`
    );
  }
  return bundlesDir;
}

export class WebviewBundlePlugin extends PluginBase<WebviewBundlePluginConfig> {
  override readonly name = 'WebviewBundlePlugin';

  constructor(config: WebviewBundlePluginConfig = {}) {
    super(config);
  }

  getHooks(): ForgeMultiHookMap {
    return {
      packageAfterCopy: [this.packageAfterCopy],
    };
  }

  packageAfterCopy = async (
    _forgeConfig: ResolvedForgeConfig,
    buildPath: string,
    _electronVersion: string,
    _platform: ForgePlatform,
    _arch: ForgeArch
  ): Promise<void> => {
    const {
      bundlesDir,
      channel,
      configFile,
      throwWhenBuiltinIsEmpty = true,
      ...inline
    } = this.config;

    const resolved = await resolveConfig({
      ...inline,
      root: inline.root ?? process.cwd(),
      configFile: configFile === true ? undefined : configFile,
    });

    if (resolved.builtin == null) {
      if (throwWhenBuiltinIsEmpty) {
        throw new Error(
          'No "builtin" config was resolved. Add a `builtin` block to your webview-bundle config ' +
            '(or pass it inline to WebviewBundlePlugin), or set `throwWhenBuiltinIsEmpty: false` to ' +
            'build without builtin bundles.'
        );
      }
      return;
    }

    const { target, outDir, include, exclude, clean } = resolved.builtin;

    // Don't mutate the resolved config: `resolved.builtin` may share its reference with `this.config`,
    // and forge invokes `packageAfterCopy` once per (platform, arch). Build a fresh target instead.
    const installTarget: BuiltinTarget =
      target == null
        ? { type: 'remote', endpoint: resolved.remote?.endpoint }
        : target.type === 'remote'
          ? { ...target, endpoint: target.endpoint ?? resolved.remote?.endpoint }
          : target;

    const dir = outDir ?? DEFAULT_BUILTIN_OUT_DIR;
    const manifest = await builtin({
      target: installTarget,
      dir,
      include: include != null ? [include] : undefined,
      exclude: exclude != null ? [exclude] : undefined,
      channel,
      clean: clean ?? true,
      write: true,
      cwd: resolved.root,
      progress: false,
    });

    if (Object.keys(manifest.entries).length === 0 && throwWhenBuiltinIsEmpty) {
      throw new Error(
        'No builtin bundles were installed (the resolved "builtin" target produced zero bundles). ' +
          'Check your target / include / exclude config, or set `throwWhenBuiltinIsEmpty: false` to ' +
          'allow an empty install.'
      );
    }

    const stageDir = path.resolve(resolved.root, dir);
    const destDir = path.resolve(
      buildPath,
      '..',
      safeBundlesDir(bundlesDir ?? DEFAULT_BUNDLES_DIR)
    );

    await fs.rm(destDir, { recursive: true, force: true });
    await fs.cp(stageDir, destDir, { recursive: true });
  };
}

export const WvbPlugin: typeof WebviewBundlePlugin = WebviewBundlePlugin;
