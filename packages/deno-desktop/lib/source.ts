// bundleSource — construct a @wvb/deno Source with Deno desktop defaults (mirrors
// @wvb/electron's source.ts):
//   • builtin bundles are READ from inside the app — `bundles` next to the entry module
//     (the `deno desktop --include`d, self-extracted dir), like Electron's `resourcesPath/bundles`.
//   • remote (downloaded) bundles are WRITTEN under the OS application-data directory, so they
//     persist across runs and app updates — like Electron's `app.getPath('userData')/bundles`.
import { fromFileUrl } from '@std/path';
import { Source, type SourceConfig } from '@wvb/deno';

export interface BundleSourceConfig extends Omit<SourceConfig, 'builtinDir' | 'remoteDir'> {
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

/**
 * The {@link SourceConfig} the defaults resolve to. Split out from {@link bundleSource} so a host
 * can reuse the directories it picked — the updater's default update filepath sits next to them.
 */
export function resolveSourceConfig(config: BundleSourceConfig = {}): SourceConfig {
  const { appName, builtinDir, remoteDir, ...rest } = config;
  return {
    ...rest,
    builtinDir: builtinDir ?? defaultBuiltinDir(),
    remoteDir: ensureDir(remoteDir ?? defaultRemoteDir(appName ?? defaultAppName())),
  };
}

export function bundleSource(config: BundleSourceConfig = {}): Source {
  return new Source(resolveSourceConfig(config));
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
  // Reading env requires `--allow-env`; a denied/missing permission must not crash the app, so each
  // read is guarded and we fall back to a cwd-local directory when env is unavailable.
  const override = tryEnv('WVB_APP_DATA_DIR');
  if (override != null && override.length > 0) {
    return override;
  }
  const home = tryEnv('HOME');
  switch (Deno.build.os) {
    case 'darwin':
      if (home != null) {
        return `${home}/Library/Application Support`;
      }
      break;
    case 'windows': {
      const appData = tryEnv('APPDATA');
      if (appData != null) {
        return appData;
      }
      const userProfile = tryEnv('USERPROFILE');
      if (userProfile != null) {
        return `${userProfile}\\AppData\\Roaming`;
      }
      break;
    }
    default: {
      const xdg = tryEnv('XDG_DATA_HOME');
      if (xdg != null) {
        return xdg;
      }
      if (home != null) {
        return `${home}/.local/share`;
      }
      break;
    }
  }
  return `${Deno.cwd()}/.wvb/app-data`;
}

function tryEnv(name: string): string | undefined {
  try {
    const value = Deno.env.get(name);
    return value != null && value.length > 0 ? value : undefined;
  } catch {
    return undefined;
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
 * Ensure the writable remote dir exists and has a manifest. The source reads remote versions from
 * `<remoteDir>/manifest.json`; seed an empty one so builtin-only apps work out of the box (an
 * existing manifest is left untouched).
 */
function ensureDir(dir: string): string {
  Deno.mkdirSync(dir, { recursive: true });
  const manifest = `${dir}/manifest.json`;
  // createNew seeds atomically: never clobber a manifest written concurrently (first-run update).
  try {
    Deno.writeTextFileSync(manifest, JSON.stringify({ manifestVersion: 1, bundles: {} }), {
      createNew: true,
    });
  } catch (error) {
    if (!(error instanceof Deno.errors.AlreadyExists)) {
      throw error;
    }
  }
  return dir;
}
