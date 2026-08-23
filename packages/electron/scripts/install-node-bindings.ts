import { existsSync } from 'node:fs';
import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';

interface NativeTarget {
  platform: NodeJS.Platform;
  arch: NodeJS.Architecture;
  /** Rust target triple, matching one of `@wvb/node`'s napi targets. */
  triple: string;
  /** napi-generated binary filename, e.g. `wvb-node.darwin-arm64.node`. */
  file: string;
  /** Suffix of the `@wvb/node-<suffix>` optional dependency (and its `npm/<suffix>` dir). */
  npmSuffix: string;
}

const NATIVE_TARGETS: readonly NativeTarget[] = [
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

const require = createRequire(import.meta.url);
const nodePkgDir = path.dirname(require.resolve('@wvb/node/package.json'));
const electronPkgDir = path.join(import.meta.dirname, '..');
const destDir = path.join(electronPkgDir, 'node-bindings');

function candidateSources(target: NativeTarget): string[] {
  const sources = [
    path.join(nodePkgDir, 'artifacts', target.file),
    path.join(nodePkgDir, 'npm', target.npmSuffix, target.file),
    path.join(nodePkgDir, target.file),
  ];
  try {
    const depDir = path.dirname(require.resolve(`@wvb/node-${target.npmSuffix}/package.json`));
    sources.push(path.join(depDir, target.file));
  } catch {
    // The optional dependency may not be installed; ignore.
  }
  return sources;
}

const strict = process.env.WVB_BUNDLE_NATIVE_STRICT === '1';

await fs.rm(destDir, { recursive: true, force: true });
await fs.mkdir(destDir, { recursive: true });

const copied: string[] = [];
const missing: NativeTarget[] = [];

for (const target of NATIVE_TARGETS) {
  const src = candidateSources(target).find(candidate => existsSync(candidate));
  if (src == null) {
    missing.push(target);
    console.warn(`[install-node-bindings] missing ${target.file} (${target.triple})`);
    continue;
  }
  await fs.copyFile(src, path.join(destDir, target.file));
  copied.push(target.file);
  console.log(`[install-node-bindings] ${target.file} <- ${path.relative(electronPkgDir, src)}`);
}

console.log(`[install-node-bindings] installed ${copied.length}/${NATIVE_TARGETS.length} binaries`);

if (strict && missing.length > 0) {
  console.error(
    `[install-node-bindings] strict mode: ${missing.length} required binaries missing: ${missing
      .map(target => target.file)
      .join(', ')}`
  );
  process.exit(1);
}
