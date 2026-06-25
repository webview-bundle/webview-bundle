// bundleSource — construct a @wvb/deno BundleSource with Deno desktop defaults (mirrors
// @wvb/electron's source.ts):
//   • builtin bundles are READ from inside the app — `bundles` next to the entry module
//     (the `deno desktop --include`d, self-extracted dir), like Electron's `resourcesPath/bundles`.
//   • remote (downloaded) bundles are WRITTEN under the OS application-data directory, so they
//     persist across runs and app updates — like Electron's `app.getPath('userData')/bundles`.
import { fromFileUrl } from '@std/path';
import { BundleSource, type BundleSourceConfig } from '@wvb/deno';

export interface SourceOptions extends Omit<BundleSourceConfig, 'builtinDir' | 'remoteDir'> {
  /**
   * Read-only builtin bundles directory. Defaults to `bundles` next to the app's entry module
   * (`Deno.mainModule`) — i.e. the `deno desktop --include`d, self-extracted dir inside the app.
   */
  builtinDir?: string;
  /**
   * Writable directory for downloaded (remote) bundles. Defaults to
   * `<app-data>/<appName>/bundles` (see {@link appDataDir}) so updates persist across runs.
   */
  remoteDir?: string;
  /** App name used in the default remote dir. Defaults to the executable name. */
  appName?: string;
}

export function bundleSource(options: SourceOptions = {}): BundleSource {
  const { appName, builtinDir, remoteDir, ...otherOptions } = options;
  return new BundleSource({
    builtinDir: builtinDir ?? defaultBuiltinDir(),
    remoteDir: ensureDir(remoteDir ?? defaultRemoteDir(appName ?? defaultAppName())),
    ...otherOptions,
  });
}

/** `bundles` next to the app's entry module — the in-app, read-only builtin bundles. */
function defaultBuiltinDir(): string {
  try {
    return fromFileUrl(new URL('./bundles', Deno.mainModule));
  } catch {
    return `${Deno.cwd()}/bundles`;
  }
}

/**
 * The OS application-data base directory where downloaded bundles persist:
 * macOS `~/Library/Application Support`, Windows `%APPDATA%`, Linux `$XDG_DATA_HOME` (or
 * `~/.local/share`). Override with the `WVB_APP_DATA_DIR` env var.
 */
export function appDataDir(): string {
  const override = Deno.env.get('WVB_APP_DATA_DIR');
  if (override != null && override.length > 0) {
    return override;
  }
  switch (Deno.build.os) {
    case 'darwin':
      return `${Deno.env.get('HOME')}/Library/Application Support`;
    case 'windows':
      return Deno.env.get('APPDATA') ?? `${Deno.env.get('USERPROFILE')}\\AppData\\Roaming`;
    default:
      return Deno.env.get('XDG_DATA_HOME') ?? `${Deno.env.get('HOME')}/.local/share`;
  }
}

function defaultRemoteDir(appName: string): string {
  return `${appDataDir()}/${appName}/bundles`;
}

function defaultAppName(): string {
  const base = Deno.execPath().split(/[/\\]/).pop() ?? 'webview-bundle';
  return base.replace(/\.exe$/i, '') || 'webview-bundle';
}

/**
 * Ensure the writable remote dir exists and has a manifest. The bundle source reads remote versions
 * from `<remoteDir>/manifest.json`; seed an empty one so builtin-only apps work out of the box
 * (an existing manifest is left untouched).
 */
function ensureDir(dir: string): string {
  Deno.mkdirSync(dir, { recursive: true });
  const manifest = `${dir}/manifest.json`;
  try {
    Deno.statSync(manifest);
  } catch {
    Deno.writeTextFileSync(manifest, JSON.stringify({ manifestVersion: 1, entries: {} }));
  }
  return dir;
}
