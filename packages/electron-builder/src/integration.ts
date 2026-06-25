import fs from 'node:fs/promises';
import path from 'node:path';
import { resolveConfig } from '@wvb/cli';
import { builtin } from '@wvb/cli/api';
import type { BuiltinTarget } from '@wvb/config';
import type { AfterPackContext, AfterPackHook, WebViewBundleOptions } from './config.js';

const DEFAULT_BUILTIN_OUT_DIR = path.join('.wvb', 'builtin', 'bundles');
const DEFAULT_BUNDLES_DIR = 'bundles';

/**
 * Resolve the packaged app's `Resources` directory from an electron-builder `afterPack` context.
 * This is where `@wvb/electron` reads builtin bundles from at runtime (`<resourcesPath>/bundles`).
 *
 * - macOS (`darwin` / `mas`): `<appOutDir>/<productFilename>.app/Contents/Resources`
 * - Windows / Linux: `<appOutDir>/resources`
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
 *
 * Most apps should prefer {@link withWebViewBundle}, which wires this hook into your config (and
 * composes with an existing `afterPack`). Use this factory directly when you compose hooks yourself,
 * or when your config references the hook by module path (`electron-builder.yml` / `package.json`):
 *
 * ```js
 * // wvb-after-pack.cjs
 * module.exports = require('@wvb/electron-builder').webViewBundleAfterPack();
 * // electron-builder.yml -> afterPack: ./wvb-after-pack.cjs
 * ```
 *
 * The hook runs once per platform/arch target, after the app is packed (so bundles land next to
 * `app.asar`, outside the archive) and before code signing (so they are signed/notarized with the
 * app). It is not invoked for `electron .` dev runs, so dev stays bundle-free.
 *
 * Note: there is no cross-target download cache, so a multi-target build (e.g. `-mwl`) installs the
 * bundles once per target. Each install cleans its own staging directory, so targets never
 * interfere.
 */
export function webViewBundleAfterPack(
  options: WebViewBundleOptions = {}
): (context: AfterPackContext) => Promise<void> {
  return async (context: AfterPackContext): Promise<void> => {
    const { bundlesDir, channel, configFile, throwWhenBuiltinIsEmpty = true, ...inline } = options;

    const resolved = await resolveConfig({
      ...inline,
      root: inline.root ?? process.cwd(),
      configFile: configFile === true ? undefined : configFile,
    });

    if (resolved.builtin == null) {
      if (throwWhenBuiltinIsEmpty) {
        throw new Error(
          'No "builtin" config was resolved. Add a `builtin` block to your webview-bundle config ' +
            '(or pass it inline to the integration), or set `throwWhenBuiltinIsEmpty: false` to ' +
            'build without builtin bundles.'
        );
      }
      return;
    }

    const { target, outDir, include, exclude, clean } = resolved.builtin;

    // Don't mutate the resolved config: `resolved.builtin` may share its reference with the inline
    // options, and electron-builder invokes `afterPack` once per (platform, arch). Build a fresh
    // target instead, defaulting a remote endpoint from `remote.endpoint` when missing.
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
          'Check your target / include / exclude config, or set `throwWhenBuiltinIsEmpty: false` ' +
          'to allow an empty install.'
      );
    }

    // `builtin()` stages bundles at `<root>/<dir>` with the exact on-disk layout the runtime expects
    // (`manifest.json` + `<name>/<name>_<version>.wvb`). Copy that tree into the packaged app's
    // resources so the runtime reads `<resourcesPath>/<bundlesDir>`.
    const stageDir = path.resolve(resolved.root, dir);
    const destDir = path.join(resolveResourcesPath(context), bundlesDir ?? DEFAULT_BUNDLES_DIR);

    await fs.rm(destDir, { recursive: true, force: true });
    await fs.cp(stageDir, destDir, { recursive: true });
  };
}

/**
 * Wrap an electron-builder configuration so it installs builtin Webview Bundles at package time.
 *
 * This is the recommended entry point — it injects the {@link webViewBundleAfterPack} hook into your
 * config, composing with any existing `afterPack` (yours runs first, then the bundle install). Your
 * config object is returned with its type preserved:
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
 *
 * Because it transforms a config object, the HoC only works with a JS/TS electron-builder config
 * file. For `electron-builder.yml` / `package.json` builds, reference {@link webViewBundleAfterPack}
 * by module path instead.
 *
 * Only a **function** `afterPack` can be composed. electron-builder also accepts a module-path
 * string for `afterPack`, but the HoC cannot faithfully resolve and invoke it, so it throws rather
 * than silently dropping your hook — convert it to a function, or call {@link webViewBundleAfterPack}
 * from within your own hook module.
 *
 * The bundles are placed in `Resources/<bundlesDir>` (default `bundles`), outside `app.asar`, so no
 * `asarUnpack` is needed and the runtime reads them directly from `process.resourcesPath`.
 */
export function withWebViewBundle<C extends object>(
  config: C,
  options: WebViewBundleOptions = {}
): C {
  const install = webViewBundleAfterPack(options);
  const existing = (config as { afterPack?: unknown }).afterPack;

  // electron-builder's `afterPack` may be `Hook | string | null`. We can only compose a function;
  // fail loud on a module-path string instead of silently discarding the user's hook.
  if (existing != null && typeof existing !== 'function') {
    throw new Error(
      `withWebViewBundle cannot compose an existing \`afterPack\` of type "${typeof existing}". ` +
        'electron-builder allows a module-path string here, but this wrapper composes function ' +
        'hooks only. Convert your `afterPack` to a function, or import `webViewBundleAfterPack()` ' +
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

export const withWvb: <C extends object>(config: C, options?: WebViewBundleOptions) => C =
  withWebViewBundle;
