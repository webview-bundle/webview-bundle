import { readFileSync } from 'node:fs';
// biome-ignore lint/correctness/useImportExtensions: JSON module import
import packageJson from '../../package.json' with { type: 'json' };

const BINARY_NAME = packageJson.napi.binaryName;

function isLinuxMusl(): boolean {
  const fromFilesystem = isMuslFromFilesystem();
  if (fromFilesystem !== null) {
    return fromFilesystem;
  }
  return isMuslFromReport() ?? false;
}

function isMuslFromFilesystem(): boolean | null {
  try {
    return readFileSync('/usr/bin/ldd', 'utf8').includes('musl');
  } catch {
    return null;
  }
}

function isMuslFromReport(): boolean | null {
  try {
    const report =
      typeof process.report?.getReport === 'function'
        ? (process.report.getReport() as {
            header?: { glibcVersionRuntime?: string };
            sharedObjects?: string[];
          })
        : null;
    if (report == null) {
      return null;
    }
    if (report.header?.glibcVersionRuntime) {
      return false;
    }
    if (Array.isArray(report.sharedObjects)) {
      return report.sharedObjects.some(o => o.includes('libc.musl-') || o.includes('ld-musl-'));
    }
    return null;
  } catch {
    return null;
  }
}

/**
 * The prebuilt `.node` filename for the current process, matching `binding.cjs`'s per-target
 * resolution (e.g. `wvb-node.darwin-arm64.node`, `wvb-node.linux-x64-musl.node`). Returns
 * `undefined` for a platform/arch `@wvb/node` does not ship a prebuilt binary for.
 *
 * Used by {@link loadBinding} to find the right binary inside an embedder-provided directory.
 */
export function resolveNativeBindingFilename(): string | undefined {
  const suffix = resolveTargetSuffix();
  return suffix != null ? `${BINARY_NAME}.${suffix}.node` : undefined;
}

function resolveTargetSuffix(): string | undefined {
  switch (process.platform) {
    case 'darwin':
      if (process.arch === 'x64') return 'darwin-x64';
      if (process.arch === 'arm64') return 'darwin-arm64';
      return undefined;
    case 'win32':
      if (process.arch === 'x64') return 'win32-x64-msvc';
      if (process.arch === 'ia32') return 'win32-ia32-msvc';
      if (process.arch === 'arm64') return 'win32-arm64-msvc';
      return undefined;
    case 'linux':
      if (process.arch === 'x64') return isLinuxMusl() ? 'linux-x64-musl' : 'linux-x64-gnu';
      if (process.arch === 'arm64') return isLinuxMusl() ? 'linux-arm64-musl' : 'linux-arm64-gnu';
      // No musl armv7 binary is shipped, so report "no prebuilt" rather than a glibc one that
      // cannot load on musl.
      if (process.arch === 'arm') return isLinuxMusl() ? undefined : 'linux-arm-gnueabihf';
      return undefined;
    case 'android':
      if (process.arch === 'arm64') return 'android-arm64';
      if (process.arch === 'arm') return 'android-arm-eabi';
      return undefined;
    default:
      return undefined;
  }
}
