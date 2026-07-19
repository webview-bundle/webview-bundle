import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { nativeTargetForCurrentProcess } from './native-targets.js';

// Point @wvb/node's napi loader at the architecture-specific binary bundled inside
// this package, so @wvb/node loads the arch @wvb/electron ships instead of relying on
// its per-arch optional dependencies (which Electron packagers routinely fail to
// hoist and unpack).
//
// `NAPI_RS_NATIVE_LIBRARY_PATH` is the override that every @napi-rs/cli-generated
// loader reads first — it is NOT scoped to @wvb/node. Leaving it set process-wide
// would make any other napi-rs addon in the app load @wvb/node's binary as its own
// (and leak into child processes), so we set it only long enough to force @wvb/node's
// binding to load, then restore it. Once binding.cjs is required, Node caches it, so
// @wvb/node's own later import reuses the binding loaded here.
const NATIVE_LIBRARY_PATH_ENV = 'NAPI_RS_NATIVE_LIBRARY_PATH';
const require = createRequire(import.meta.url);

function bundledNativeDir(): string {
  return path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'native');
}

// The bundled Linux binaries are glibc-only; on musl (Alpine) forcing a glibc binary
// would break loading, so leave the override unset and let @wvb/node resolve its own
// musl build.
function isLinuxMusl(): boolean {
  if (process.platform !== 'linux') {
    return false;
  }
  try {
    const report =
      typeof process.report?.getReport === 'function' ? process.report.getReport() : null;
    const header = (report as { header?: { glibcVersionRuntime?: string } } | null)?.header;
    if (header?.glibcVersionRuntime) {
      return false;
    }
    const sharedObjects = (report as { sharedObjects?: string[] } | null)?.sharedObjects;
    if (Array.isArray(sharedObjects)) {
      return sharedObjects.some(o => o.includes('libc.musl-') || o.includes('ld-musl-'));
    }
  } catch {
    // Fall through and assume glibc.
  }
  return false;
}

function forceLoadNodeBinding(bindingPath: string): void {
  const had = Object.hasOwn(process.env, NATIVE_LIBRARY_PATH_ENV);
  const previous = process.env[NATIVE_LIBRARY_PATH_ENV];
  process.env[NATIVE_LIBRARY_PATH_ENV] = bindingPath;
  try {
    const nodePkg = require.resolve('@wvb/node/package.json');
    require(path.join(path.dirname(nodePkg), 'binding.cjs'));
  } catch {
    // Best effort: if the bundled binary or @wvb/node cannot be loaded here, restore
    // the env below and let @wvb/node fall back to its own resolution on import.
  } finally {
    if (had) {
      process.env[NATIVE_LIBRARY_PATH_ENV] = previous;
    } else {
      delete process.env[NATIVE_LIBRARY_PATH_ENV];
    }
  }
}

const overrideAlreadySet =
  process.env[NATIVE_LIBRARY_PATH_ENV] != null && process.env[NATIVE_LIBRARY_PATH_ENV] !== '';
const target = nativeTargetForCurrentProcess();

if (!overrideAlreadySet && target != null && !isLinuxMusl()) {
  const bundled = path.join(bundledNativeDir(), target.file);
  if (existsSync(bundled)) {
    forceLoadNodeBinding(bundled);
  }
}
