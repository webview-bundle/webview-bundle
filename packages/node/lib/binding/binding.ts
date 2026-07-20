import { statSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { buildApi, type WvbNodeBinding } from '../api.js';
import { resolveNativeBindingFilename } from './binding-target.js';

let cached: WvbNodeBinding | undefined;

/**
 * Load `@wvb/node`'s native binding and return its wrapped runtime API — the same surface as
 * `import * as wvbNode from '@wvb/node'`.
 *
 * `dirOrFile` is either:
 * - a **directory**: `loadBinding` picks the `<binaryName>.<target>.node` file inside it that matches
 *   the current `process.platform`/`process.arch` (the same per-target names `binding.cjs` resolves),
 *   e.g. `wvb-node.darwin-arm64.node`; or
 * - a **`.node` file**: it is loaded directly.
 *
 * ```ts
 * import { loadBinding } from '@wvb/node/binding';
 *
 * const wvbNode = loadBinding(nodeBindingsDir);
 * const source = new wvbNode.BundleSource({ builtinDir, remoteDir });
 * ```
 *
 * Use this to load a binary you ship yourself instead of relying on `@wvb/node`'s per-arch optional
 * dependencies (which Electron and other packagers routinely fail to unpack).
 *
 * The native binding is a process-wide singleton: the first `loadBinding` call decides which binary
 * loads, and later calls return that same instance.
 */
export function loadBinding(dirOrFile: string): WvbNodeBinding {
  if (cached != null) {
    return cached;
  }
  const nodeRequire = createRequire(import.meta.url);
  cached = buildApi(nodeRequire(resolveBindingFile(dirOrFile)));
  return cached;
}

function resolveBindingFile(dirOrFile: string): string {
  if (!isDirectory(dirOrFile)) {
    return dirOrFile;
  }
  const filename = resolveNativeBindingFilename();
  if (filename == null) {
    throw new Error(
      `@wvb/node: no prebuilt binding for ${process.platform}-${process.arch}; ` +
        'pass an explicit .node file path to loadBinding instead of a directory.'
    );
  }
  return path.join(dirOrFile, filename);
}

function isDirectory(target: string): boolean {
  return statSync(target, { throwIfNoEntry: false })?.isDirectory() ?? false;
}
