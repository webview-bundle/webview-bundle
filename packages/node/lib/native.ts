/**
 * Name of the environment variable the napi loader (`binding.cjs`) checks before
 * every other native-binding resolution strategy. Set it to an absolute path to a
 * `.node` file to force `@wvb/node` to load that binary instead of resolving one of
 * the per-arch optional dependencies.
 *
 * This is the supported hook for embedders that ship their own copy of the native
 * binaries — e.g. `@wvb/electron`, which bundles every desktop arch and points this
 * at the one matching the current process. Because `binding.cjs` reads it while the
 * binding loads (i.e. when `@wvb/node` is first imported), it must be set before that
 * first import to take effect.
 */
export const NATIVE_BINDING_PATH_ENV = 'NAPI_RS_NATIVE_LIBRARY_PATH';

/** The native-binding path override currently in effect, if any. */
export function getNativeBindingPath(): string | undefined {
  const value = process.env[NATIVE_BINDING_PATH_ENV];
  return value != null && value !== '' ? value : undefined;
}
