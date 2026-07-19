import fs from 'node:fs/promises';
import path from 'node:path';
import { resolveConfig } from '@wvb/cli';
import { builtin } from '@wvb/cli/api';
import type { BuiltinTarget } from '@wvb/config';
import type { AfterPackContext, AfterPackHook, WebviewBundleOptions } from './config.js';

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

/**
 * Resolve the packaged app's `Resources` directory from an electron-builder `afterPack` context.
 */
export function resolveResourcesPath(context: AfterPackContext): string {
  const { appOutDir, electronPlatformName } = context;
  if (electronPlatformName === 'darwin' || electronPlatformName === 'mas') {
    const appName = `${context.packager.appInfo.productFilename}.app`;
    return path.join(appOutDir, appName, 'Contents', 'Resources');
  }
  return path.join(appOutDir, 'resources');
}

/**
 * Build an electron-builder `afterPack` hook that installs builtin Webview Bundles — downloaded from
 * the remote and/or packed from local workspaces as configured in your webview-bundle config — and
 * embeds them into the packaged app's `Resources/<bundlesDir>`, where `@wvb/electron` looks for them
 * at runtime.
 */
export function webviewBundleAfterPack(
  options: WebviewBundleOptions = {}
): (context: AfterPackContext) => Promise<void> {
  return async (context: AfterPackContext): Promise<void> => {
    const { bundlesDir, channel, configFile, throwWhenBuiltinIsEmpty = true, ...inline } = options;

    const resolved = await resolveConfig({
      ...inline,
      root: inline.root ?? context.packager.projectDir ?? process.cwd(),
      configFile: configFile === true ? undefined : configFile,
    });

    const { target, outDir, include, exclude, clean } = resolved.builtin ?? {};

    // Don't mutate the resolved config: `resolved.builtin` may share its reference with the inline
    // options, and electron-builder invokes `afterPack` once per (platform, arch). Build a fresh
    // target instead, defaulting a remote endpoint from `remote.endpoint` when missing.
    const installTarget: BuiltinTarget =
      target == null
        ? { type: 'remote', endpoint: resolved.remote?.endpoint }
        : target.type === 'remote'
          ? { ...target, endpoint: target.endpoint ?? resolved.remote?.endpoint }
          : target;

    // Stage per (platform, arch) so a multi-target build never shares — and clobbers — one dir.
    const dir = path.join(
      outDir ?? DEFAULT_BUILTIN_OUT_DIR,
      `${context.electronPlatformName}-${context.arch}`
    );
    const { manifest } = await builtin({
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
          'Check your target / include / exclude config, or set `throwWhenBuiltinIsEmpty: false` ' +
          'to allow an empty install.'
      );
    }

    const stageDir = path.resolve(resolved.root, dir);
    const destDir = path.join(
      resolveResourcesPath(context),
      safeBundlesDir(bundlesDir ?? DEFAULT_BUNDLES_DIR)
    );

    await fs.rm(destDir, { recursive: true, force: true });
    await fs.cp(stageDir, destDir, { recursive: true });
  };
}

export const wvbAfterPack: typeof webviewBundleAfterPack = webviewBundleAfterPack;

/**
 * Wrap an electron-builder configuration so it installs builtin Webview Bundles at package time.
 *
 * ```ts
 * // electron-builder.config.ts
 * import { withWebViewBundle } from '@wvb/electron-builder';
 *
 * export default withWebViewBundle({
 *   appId: 'com.example.app',
 *   asar: true,
 *   mac: { target: 'dmg' },
 * });
 * ```
 */
export function withWebviewBundle<C extends object>(
  config: C,
  options: WebviewBundleOptions = {}
): C {
  const install = webviewBundleAfterPack(options);
  const existing = (config as { afterPack?: unknown }).afterPack;

  // electron-builder's `afterPack` may be `Hook | string | null`. We can only compose a function;
  // fail loud on a module-path string instead of silently discarding the user's hook.
  if (existing != null && typeof existing !== 'function') {
    throw new Error(
      `\`withWebviewBundle\` cannot compose an existing \`afterPack\` of type "${typeof existing}". ` +
        'electron-builder allows a module-path string here, but this wrapper composes function ' +
        'hooks only. Convert your `afterPack` to a function, or import `webviewBundleAfterPack()` ' +
        'from your own hook module and call it there.'
    );
  }

  const afterPack: AfterPackHook = async (context: AfterPackContext): Promise<void> => {
    if (typeof existing === 'function') {
      await (existing as AfterPackHook)(context);
    }
    await install(context);
  };

  return { ...config, afterPack } as C;
}

export const withWvb: typeof withWebviewBundle = withWebviewBundle;
