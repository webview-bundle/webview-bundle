import fs from 'node:fs/promises';
import path from 'node:path';
import { formatVersion, type VersionMap } from './versions.js';

export interface RenderContext {
  readonly projectName: string;
  readonly bundleName: string;
  readonly pm: string;
  readonly pmRun: string;
  readonly versions: VersionMap;
}

export interface FileWrite {
  readonly path: string;
  readonly contents: Buffer;
  readonly mode: number;
}

const PACKAGE_MANIFEST = '_package.json';

/**
 * Substitution is opt-in by extension rather than by sniffing for binary content: a wrong guess on a
 * format added later would silently corrupt it, whereas a missing extension here fails visibly.
 */
export const SUBSTITUTABLE = new Set([
  '.json',
  '.md',
  '.ts',
  '.tsx',
  '.js',
  '.mjs',
  '.cjs',
  '.html',
  '.css',
  '.toml',
  '.yml',
  '.yaml',
  '.kt',
  '.kts',
  '.swift',
  '.gradle',
  '.xml',
  '.rs',
  '.sh',
  '.txt',
]);

const DEPENDENCY_FIELDS = new Set([
  'dependencies',
  'devDependencies',
  'peerDependencies',
  'optionalDependencies',
]);

const MANIFEST_KEY_ORDER = [
  'name',
  'version',
  'private',
  'description',
  'license',
  'type',
  'main',
  'module',
  'types',
  'exports',
  'bin',
  'files',
  'workspaces',
  'scripts',
  'dependencies',
  'devDependencies',
  'peerDependencies',
  'optionalDependencies',
  'engines',
];

const TOKEN = /\{\{\s*([a-zA-Z][a-zA-Z0-9]*)\s*(?::\s*([^}]+?)\s*)?\}\}/g;

export function substitute(content: string, ctx: RenderContext, source: string): string {
  return content.replace(TOKEN, (_match, name: string, arg?: string) => {
    switch (name) {
      case 'projectName':
        return ctx.projectName;
      case 'bundleName':
        return ctx.bundleName;
      case 'pm':
        return ctx.pm;
      case 'pmRun':
        return ctx.pmRun;
      case 'wvbVersion': {
        if (arg == null) {
          throw new Error(
            `${source}: "{{wvbVersion}}" needs a package, e.g. {{wvbVersion:@wvb/cli}}.`
          );
        }
        const version = ctx.versions[arg];
        if (version == null) {
          throw new Error(
            `${source}: no resolved version for "${arg}" in {{wvbVersion:${arg}}}. Resolved: ${Object.keys(ctx.versions).join(', ') || '(none)'}`
          );
        }
        return formatVersion(arg, version);
      }
      default:
        throw new Error(`${source}: unknown token "{{${name}}}".`);
    }
  });
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return value != null && typeof value === 'object' && !Array.isArray(value);
}

export function mergeManifest(
  base: Record<string, unknown>,
  overlay: Record<string, unknown>
): Record<string, unknown> {
  const merged: Record<string, unknown> = { ...base };
  for (const [key, value] of Object.entries(overlay)) {
    const existing = merged[key];
    merged[key] =
      isPlainObject(existing) && isPlainObject(value) ? mergeManifest(existing, value) : value;
  }
  return merged;
}

function formatManifest(manifest: Record<string, unknown>): string {
  const ordered: Record<string, unknown> = {};
  const keys = Object.keys(manifest).sort((a, b) => {
    const ai = MANIFEST_KEY_ORDER.indexOf(a);
    const bi = MANIFEST_KEY_ORDER.indexOf(b);
    if (ai === -1 && bi === -1) {
      return a.localeCompare(b);
    }
    if (ai === -1) {
      return 1;
    }
    if (bi === -1) {
      return -1;
    }
    return ai - bi;
  });
  for (const key of keys) {
    const value = manifest[key];
    ordered[key] =
      DEPENDENCY_FIELDS.has(key) && isPlainObject(value)
        ? Object.fromEntries(Object.entries(value).sort(([a], [b]) => a.localeCompare(b)))
        : value;
  }
  return `${JSON.stringify(ordered, null, 2)}\n`;
}

/** `_package.json` -> `package.json`, `_gitignore` -> `.gitignore`. */
function outputName(name: string): string {
  if (name === PACKAGE_MANIFEST) {
    return 'package.json';
  }
  return name.startsWith('_') ? `.${name.slice(1)}` : name;
}

async function walk(dir: string, prefix = ''): Promise<string[]> {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const rel = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
    if (entry.isDirectory()) {
      files.push(...(await walk(path.join(dir, entry.name), rel)));
    } else if (entry.isFile()) {
      files.push(rel);
    }
  }
  return files;
}

function outputPath(relative: string): string {
  return relative
    .split('/')
    .map(segment => outputName(segment))
    .join('/');
}

export async function planFiles(
  templatesDir: string,
  layers: readonly string[],
  ctx: RenderContext
): Promise<readonly FileWrite[]> {
  const files = new Map<string, FileWrite>();
  const manifests = new Map<string, Record<string, unknown>[]>();

  for (const layer of layers) {
    const layerDir = path.join(templatesDir, layer);
    for (const relative of await walk(layerDir)) {
      const absolute = path.join(layerDir, relative);
      const source = `${layer}/${relative}`;
      const raw = await fs.readFile(absolute);
      const { mode } = await fs.stat(absolute);

      if (path.basename(relative) === PACKAGE_MANIFEST) {
        const key = outputPath(relative);
        const collected = manifests.get(key) ?? [];
        collected.push(JSON.parse(substitute(raw.toString('utf8'), ctx, source)));
        manifests.set(key, collected);
        continue;
      }

      const contents = SUBSTITUTABLE.has(path.extname(relative))
        ? Buffer.from(substitute(raw.toString('utf8'), ctx, source), 'utf8')
        : raw;
      files.set(outputPath(relative), { path: outputPath(relative), contents, mode });
    }
  }

  for (const [key, collected] of manifests) {
    const merged = collected.reduce((acc, manifest) => mergeManifest(acc, manifest), {});
    files.set(key, {
      path: key,
      contents: Buffer.from(formatManifest(merged), 'utf8'),
      mode: 0o644,
    });
  }

  return [...files.values()].sort((a, b) => a.path.localeCompare(b.path));
}

export async function materialize(
  writes: readonly FileWrite[],
  targetDir: string,
  options: { readonly dryRun?: boolean } = {}
): Promise<void> {
  for (const write of writes) {
    const destination = path.join(targetDir, write.path);
    if (options.dryRun === true) {
      continue;
    }
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.writeFile(destination, write.contents, { mode: write.mode });
  }
}
