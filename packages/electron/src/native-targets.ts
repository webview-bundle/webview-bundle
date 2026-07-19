export interface NativeTarget {
  platform: NodeJS.Platform;
  arch: NodeJS.Architecture;
  /** Rust target triple, matching one of `@wvb/node`'s napi targets. */
  triple: string;
  /** napi-generated binary filename, e.g. `wvb-node.darwin-arm64.node`. */
  file: string;
  /** Suffix of the `@wvb/node-<suffix>` optional dependency (and its `npm/<suffix>` dir). */
  npmSuffix: string;
}

// Electron only ships glibc (linux), darwin and win32 binaries, so the musl,
// android, freebsd and armv7 targets that `@wvb/node` also publishes are left
// out here on purpose: bundling them would only add weight for binaries Electron
// can never load.
export const NATIVE_TARGETS: readonly NativeTarget[] = [
  {
    platform: 'darwin',
    arch: 'x64',
    triple: 'x86_64-apple-darwin',
    file: 'wvb-node.darwin-x64.node',
    npmSuffix: 'darwin-x64',
  },
  {
    platform: 'darwin',
    arch: 'arm64',
    triple: 'aarch64-apple-darwin',
    file: 'wvb-node.darwin-arm64.node',
    npmSuffix: 'darwin-arm64',
  },
  {
    platform: 'win32',
    arch: 'x64',
    triple: 'x86_64-pc-windows-msvc',
    file: 'wvb-node.win32-x64-msvc.node',
    npmSuffix: 'win32-x64-msvc',
  },
  {
    platform: 'win32',
    arch: 'arm64',
    triple: 'aarch64-pc-windows-msvc',
    file: 'wvb-node.win32-arm64-msvc.node',
    npmSuffix: 'win32-arm64-msvc',
  },
  {
    platform: 'win32',
    arch: 'ia32',
    triple: 'i686-pc-windows-msvc',
    file: 'wvb-node.win32-ia32-msvc.node',
    npmSuffix: 'win32-ia32-msvc',
  },
  {
    platform: 'linux',
    arch: 'x64',
    triple: 'x86_64-unknown-linux-gnu',
    file: 'wvb-node.linux-x64-gnu.node',
    npmSuffix: 'linux-x64-gnu',
  },
  {
    platform: 'linux',
    arch: 'arm64',
    triple: 'aarch64-unknown-linux-gnu',
    file: 'wvb-node.linux-arm64-gnu.node',
    npmSuffix: 'linux-arm64-gnu',
  },
];

export function nativeTargetForCurrentProcess(): NativeTarget | undefined {
  return NATIVE_TARGETS.find(
    target => target.platform === process.platform && target.arch === process.arch
  );
}
