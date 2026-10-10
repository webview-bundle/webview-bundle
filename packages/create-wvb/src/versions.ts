import fs from 'node:fs/promises';

export type VersionMap = Record<string, string>;

export type RegistryKind = 'npm' | 'jsr' | 'crates' | 'maven' | 'github-tag';

export interface RegistryEntry {
  readonly kind: RegistryKind;
  readonly npm?: string;
  readonly jsr?: string;
  readonly crate?: string;
  readonly maven?: { readonly group: string; readonly artifact: string };
  readonly github?: { readonly owner: string; readonly repo: string };
}

/** The registry each `{{wvbVersion:<pkg>}}` token resolves against. */
export const REGISTRIES: Record<string, RegistryEntry> = {
  '@wvb/bridge': { kind: 'npm', npm: '@wvb/bridge' },
  '@wvb/cli': { kind: 'npm', npm: '@wvb/cli' },
  '@wvb/config': { kind: 'npm', npm: '@wvb/config' },
  '@wvb/electron': { kind: 'npm', npm: '@wvb/electron' },
  '@wvb/electron-forge': { kind: 'npm', npm: '@wvb/electron-forge' },
  '@wvb/electron-builder': { kind: 'npm', npm: '@wvb/electron-builder' },
  '@wvb/deno': { kind: 'jsr', jsr: '@wvb/deno' },
  '@wvb/deno-desktop': { kind: 'jsr', jsr: '@wvb/deno-desktop' },
  'wvb-tauri': { kind: 'crates', crate: 'wvb-tauri' },
  'webview-bundle-android': {
    kind: 'maven',
    maven: { group: 'dev.wvb', artifact: 'webview-bundle-android' },
  },
  'webview-bundle-ios': {
    kind: 'github-tag',
    github: { owner: 'webview-bundle', repo: 'webview-bundle-ios' },
  },
};

const USER_AGENT = 'create-wvb (+https://github.com/webview-bundle/webview-bundle)';
const TIMEOUT_MS = 8000;

/**
 * A caret range on a prerelease resolves *upward* into the stable that supersedes it, so prereleases
 * pin exact; only stable versions get a caret. Non-semver values (a `file:`/`npm:` override) pass
 * through untouched.
 */
export function toRange(version: string): string {
  if (!/^\d+\.\d+\.\d+(?:[-+].*)?$/.test(version)) {
    return version;
  }
  return /[-+]/.test(version) ? version : `^${version}`;
}

/**
 * The form written into a template for a resolved version. npm/JSR ranges get a caret; crates, Maven
 * and SPM take a bare version that the template wraps in its own syntax (Cargo reads `"1.2.3"` as a
 * caret; SPM `upToNextMajor(from:)` and a Maven coordinate want the bare number).
 */
export function formatVersion(pkg: string, version: string): string {
  const kind = REGISTRIES[pkg]?.kind;
  return kind === 'npm' || kind === 'jsr' ? toRange(version) : version;
}

async function fetchJson(url: string, headers: Record<string, string> = {}): Promise<unknown> {
  try {
    const res = await fetch(url, {
      headers: { 'user-agent': USER_AGENT, ...headers },
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    if (!res.ok) {
      return null;
    }
    return await res.json();
  } catch {
    return null;
  }
}

function asString(value: unknown): string | null {
  return typeof value === 'string' && value.length > 0 ? value : null;
}

function compareSemver(a: string, b: string): number {
  const parse = (v: string) => v.replace(/^v/, '').split(/[.+-]/).map(Number);
  const [pa, pb] = [parse(a), parse(b)];
  for (let i = 0; i < 3; i++) {
    const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (!Number.isNaN(diff) && diff !== 0) {
      return diff;
    }
  }
  return 0;
}

async function resolveEntry(entry: RegistryEntry): Promise<string | null> {
  switch (entry.kind) {
    case 'npm': {
      const json = await fetchJson(`https://registry.npmjs.org/${entry.npm}/latest`);
      return asString((json as { version?: unknown } | null)?.version);
    }
    case 'jsr': {
      const json = await fetchJson(`https://jsr.io/${entry.jsr}/meta.json`);
      return asString((json as { latest?: unknown } | null)?.latest);
    }
    case 'crates': {
      const json = await fetchJson(`https://crates.io/api/v1/crates/${entry.crate}`);
      const crate = (
        json as { crate?: { max_stable_version?: unknown; newest_version?: unknown } } | null
      )?.crate;
      return asString(crate?.max_stable_version) ?? asString(crate?.newest_version);
    }
    case 'maven': {
      const query = encodeURIComponent(
        `g:"${entry.maven?.group}" AND a:"${entry.maven?.artifact}"`
      );
      const json = await fetchJson(
        `https://search.maven.org/solrsearch/select?q=${query}&core=gav&rows=1&wt=json`
      );
      const doc = (json as { response?: { docs?: Array<{ v?: unknown }> } } | null)?.response
        ?.docs?.[0];
      return asString(doc?.v);
    }
    case 'github-tag': {
      const { owner, repo } = entry.github ?? { owner: '', repo: '' };
      const json = await fetchJson(
        `https://api.github.com/repos/${owner}/${repo}/tags?per_page=100`,
        {
          accept: 'application/vnd.github+json',
        }
      );
      if (!Array.isArray(json)) {
        return null;
      }
      const versions = json
        .map(tag => asString((tag as { name?: unknown }).name))
        .filter((name): name is string => name != null && /^v?\d+\.\d+\.\d+$/.test(name))
        .sort(compareSemver);
      const latest = versions.at(-1);
      return latest == null ? null : latest.replace(/^v/, '');
    }
  }
}

export async function loadVersionOverrides(file: string): Promise<VersionMap> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(await fs.readFile(file, 'utf8'));
  } catch (error) {
    throw new Error(`Could not read version overrides from "${file}": ${(error as Error).message}`);
  }
  if (parsed == null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new Error(
      `Version overrides in "${file}" must be a JSON object of { "package": "version" }.`
    );
  }
  const overrides: VersionMap = {};
  for (const [name, version] of Object.entries(parsed as Record<string, unknown>)) {
    if (typeof version !== 'string') {
      throw new Error(`Version override for "${name}" must be a string, got ${typeof version}.`);
    }
    overrides[name] = version;
  }
  return overrides;
}

/**
 * Resolves the latest published version of each package from its registry. An override file (or the
 * `WVB_TEMPLATE_VERSIONS` env var) wins and skips the network — this is how CI and offline runs pin
 * versions. Packages that cannot be resolved are simply absent from the returned map.
 */
export async function resolveVersions(
  packages: readonly string[],
  overrideFile?: string
): Promise<VersionMap> {
  const file = overrideFile ?? process.env.WVB_TEMPLATE_VERSIONS;
  const overrides = file == null || file === '' ? {} : await loadVersionOverrides(file);

  const result: VersionMap = {};
  await Promise.all(
    packages.map(async pkg => {
      if (overrides[pkg] != null) {
        result[pkg] = overrides[pkg];
        return;
      }
      const entry = REGISTRIES[pkg];
      if (entry == null) {
        throw new Error(`No registry is configured for "${pkg}".`);
      }
      const version = await resolveEntry(entry);
      if (version != null) {
        result[pkg] = version;
      }
    })
  );
  return result;
}
