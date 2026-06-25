import fs from 'node:fs/promises';
import path from 'node:path';
import { pathExists } from './fs.js';

/**
 * Sub-directory under the Tauri app's Resource directory that the `@wvb/tauri` runtime reads builtin
 * bundles from by default (`BaseDirectory::Resource` + `"bundles"`).
 */
export const TAURI_BUNDLES_DIR = 'bundles';

const TAURI_CONFIG_FILES = ['tauri.conf.json', 'tauri.conf.json5', 'Tauri.toml'];

export interface TauriProject {
  /** Absolute path to the Tauri project directory (the one containing `tauri.conf.*`, usually `src-tauri`). */
  dir: string;
  /** Absolute path to the resolved Tauri config file. */
  configFile: string;
}

/**
 * Locate a Tauri project by finding its config file. When `explicitDir` is given it is used directly;
 * otherwise this searches `cwd`, `cwd/src-tauri`, and a few parent levels — so it works whether
 * invoked from the project root or from a `beforeBundleCommand` whose working directory is the
 * frontend dir, not `src-tauri`.
 */
export async function resolveTauriProject(
  cwd: string,
  explicitDir?: string
): Promise<TauriProject | null> {
  const candidateDirs: string[] = [];
  if (explicitDir != null) {
    candidateDirs.push(path.resolve(cwd, explicitDir));
  } else {
    let dir = path.resolve(cwd);
    for (let i = 0; i < 4; i++) {
      candidateDirs.push(path.join(dir, 'src-tauri'));
      candidateDirs.push(dir);
      const parent = path.dirname(dir);
      if (parent === dir) {
        break;
      }
      dir = parent;
    }
  }

  for (const candidate of candidateDirs) {
    for (const file of TAURI_CONFIG_FILES) {
      const configFile = path.join(candidate, file);
      if (await pathExists(configFile)) {
        return { dir: candidate, configFile };
      }
    }
  }
  return null;
}

export type BundleResourcesStatus = 'ok' | 'missing' | 'skipped';

/**
 * Best-effort check that a Tauri config's `bundle.resources` declares the staged builtin bundles
 * directory, so the bundler actually ships them. Only JSON/JSON5 configs are inspected; TOML (or an
 * unparsable config) is skipped. Returns:
 * - `'ok'`: a `resources` entry references `bundlesDir`.
 * - `'missing'`: `resources` is absent, or has no entry referencing `bundlesDir`.
 * - `'skipped'`: the config could not be parsed (e.g. TOML, or comments/trailing commas).
 */
export async function checkBundleResources(
  configFile: string,
  bundlesDir: string = TAURI_BUNDLES_DIR
): Promise<BundleResourcesStatus> {
  if (configFile.endsWith('.toml')) {
    return 'skipped';
  }
  let conf: unknown;
  try {
    const raw = await fs.readFile(configFile, 'utf8');
    conf = JSON.parse(raw);
  } catch {
    return 'skipped';
  }

  const resources = (conf as { bundle?: { resources?: unknown } } | null)?.bundle?.resources;
  if (resources == null) {
    return 'missing';
  }

  const entries = Array.isArray(resources)
    ? resources
    : typeof resources === 'object'
      ? Object.keys(resources as Record<string, unknown>)
      : [];
  // Match on a path segment (not a substring) so `bundles/**/*.wvb` and `bundles/` match while
  // `mybundles/x` does not.
  const referencesBundles = entries.some(
    entry => typeof entry === 'string' && entry.split(/[/\\]/).includes(bundlesDir)
  );
  return referencesBundles ? 'ok' : 'missing';
}
