/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import { statSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import type * as binding from '../../binding.cjs';
import { resolveNativeBindingFilename } from './binding-target.js';

/** Everything the native binding exports — the same surface as `import * as wvbNode from '@wvb/node'`. */
export type WvbNodeBinding = typeof binding;

let cached: WvbNodeBinding | undefined;

/**
 * Load `@wvb/node`'s native binding.
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
 * const source = new wvbNode.Source({ builtinDir, remoteDir });
 * ```
 */
export function loadBinding(dirOrFile: string): WvbNodeBinding {
  if (cached != null) {
    return cached;
  }
  const nodeRequire = createRequire(import.meta.url);
  cached = nodeRequire(resolveBindingFile(dirOrFile)) as WvbNodeBinding;
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
