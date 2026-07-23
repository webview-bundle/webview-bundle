import { readFileSync } from 'node:fs';
// biome-ignore lint/correctness/useImportExtensions: JSON module import
import { napi } from '../../package.json' with { type: 'json' };

const BINARY_NAME = napi.binaryName;

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

const RUST_ARCH_TO_NODE: Record<string, NodeJS.Architecture> = {
  x86_64: 'x64',
  aarch64: 'arm64',
  i686: 'ia32',
  armv7: 'arm',
};

interface NapiTarget {
  platform: NodeJS.Platform;
  arch: NodeJS.Architecture;
  musl: boolean;
  /** napi's `<platform>-<arch>[-<abi>]` `.node` filename suffix, e.g. `linux-x64-gnu`. */
  suffix: string;
}

// Parse a Rust target triple from `napi.targets` into the Node platform/arch/libc it runs on and the
// suffix napi names its `.node` after. Returns null for a triple this loader cannot map.
function parseNapiTarget(triple: string): NapiTarget | null {
  const arch = RUST_ARCH_TO_NODE[triple.split('-')[0] ?? ''];
  if (arch == null) {
    return null;
  }
  if (triple.includes('apple-darwin')) {
    return { platform: 'darwin', arch, musl: false, suffix: `darwin-${arch}` };
  }
  if (triple.includes('windows')) {
    return { platform: 'win32', arch, musl: false, suffix: `win32-${arch}-msvc` };
  }
  // Check `android` before `linux`: android triples (e.g. `aarch64-linux-android`) also contain
  // `linux`.
  if (triple.includes('android')) {
    return {
      platform: 'android',
      arch,
      musl: false,
      suffix: arch === 'arm' ? 'android-arm-eabi' : `android-${arch}`,
    };
  }
  if (triple.includes('linux')) {
    const abi = triple.split('-').at(-1) ?? '';
    return {
      platform: 'linux',
      arch,
      musl: triple.includes('musl'),
      suffix: `linux-${arch}-${abi}`,
    };
  }
  return null;
}

function resolveTargetSuffix(): string | undefined {
  const musl = process.platform === 'linux' && isLinuxMusl();
  for (const triple of napi.targets) {
    const target = parseNapiTarget(triple);
    if (
      target != null &&
      target.platform === process.platform &&
      target.arch === process.arch &&
      target.musl === musl
    ) {
      return target.suffix;
    }
  }
  return undefined;
}
