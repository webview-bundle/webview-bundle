import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { isUnreleased, type VersionMap } from './versions.js';

export type TemplateStatus = 'stable' | 'caveat' | 'experimental';

export interface Template {
  readonly name: string;
  readonly description: string;
  readonly hint: string;
  readonly status: TemplateStatus;
  readonly layers: readonly string[];
  readonly caveats?: readonly string[];
  readonly nextSteps?: readonly string[];
}

export type TemplateManifest = Record<string, Template>;

const VERSION_TOKEN = /\{\{\s*wvbVersion\s*:\s*([^}]+?)\s*\}\}/g;

export function templatesDir(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'templates');
}

export async function loadManifest(dir = templatesDir()): Promise<TemplateManifest> {
  const file = path.join(dir, 'templates.json');
  const content = await fs.readFile(file, 'utf8');
  return JSON.parse(content) as TemplateManifest;
}

async function walk(dir: string): Promise<string[]> {
  const out: string[] = [];
  const entries = await fs.readdir(dir, { withFileTypes: true }).catch(() => []);
  for (const entry of entries) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      out.push(...(await walk(full)));
    } else {
      out.push(full);
    }
  }
  return out;
}

/** The `{{wvbVersion:<pkg>}}` packages referenced anywhere in a set of layers. */
export async function collectPackages(
  dir: string,
  layers: readonly string[]
): Promise<readonly string[]> {
  const found = new Set<string>();
  for (const layer of layers) {
    for (const file of await walk(path.join(dir, layer))) {
      const raw = await fs.readFile(file, 'utf8').catch(() => '');
      for (const match of raw.matchAll(VERSION_TOKEN)) {
        found.add(match[1] as string);
      }
    }
  }
  return [...found];
}

/** The referenced packages whose latest published version is missing or a 0.0.0 placeholder. */
export function unreleasedPackages(
  packages: readonly string[],
  versions: VersionMap
): readonly string[] {
  return packages.filter(pkg => isUnreleased(versions[pkg]));
}

export function collectCaveats(template: Template): readonly string[] {
  return template.caveats ?? [];
}
