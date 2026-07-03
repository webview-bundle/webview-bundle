import type { Version } from './version.ts';

/**
 * Everything registry-specific in one place, per registry: how to publish a version, how to tell
 * whether a version is already live, how to recognize the registry rejecting a duplicate, and the
 * public URL of a published version. The existence checks answer `null` when they cannot tell
 * (network error, unexpected status); the caller should then attempt the publish and let
 * `isDuplicateRejection` classify the outcome.
 */
export interface Registry {
  readonly type: RegistryType;
  /** Public URL of a published version. */
  url(name: string, version: string): string;
  /** Whether `name@version` is already live. `null` when the check itself failed. */
  exists(name: string, version: string): Promise<boolean | null>;
  /**
   * Whether a failed publish command's output is the registry rejecting an already-existing
   * version. Backstops {@link exists} for versions it cannot see — notably npm's *staged*
   * publishes (stable channel), which reserve the version but stay hidden until approved.
   */
  isDuplicateRejection(output: string): boolean;
  /** The command that publishes `name@version` from the manifest's directory. */
  publishCommand(opts: PublishCommandOptions): PublishCommand;
}

export type RegistryType = 'npm' | 'cargo';

export interface PublishCommandOptions {
  name: string;
  /** Repo-relative directory of the manifest. */
  dir: string;
  version: Version;
  distTag?: string;
}

export interface PublishCommand {
  cmd: string;
  args: string[];
  /** Repo-relative working directory. */
  path: string;
}

const FETCH_TIMEOUT_MS = 10_000;

export const npmRegistry: Registry = {
  type: 'npm',

  url(name, version) {
    return `https://www.npmjs.com/package/${name}/v/${version}`;
  },

  async exists(name, version) {
    // Scoped names keep the `@` but encode the `/` (`@scope%2Fname`).
    const url = `https://registry.npmjs.org/${name.replace('/', '%2F')}/${version}`;
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) });
      if (res.ok) {
        return true;
      }
      if (res.status === 404) {
        return false;
      }
      return null;
    } catch {
      return null;
    }
  },

  isDuplicateRejection(output) {
    // npm E403: "You cannot publish over the previously published versions"
    return /cannot publish over/i.test(output) || /EPUBLISHCONFLICT/i.test(output);
  },

  publishCommand({ dir, version, distTag }) {
    const args = ['npm', 'publish', '--access=public', '--provenance'];
    const prerelease = version.prerelease;
    if (prerelease != null) {
      // Prereleases publish under their channel id (e.g. `next`).
      args.push(`--tag=${prerelease.id}`);
    } else if (distTag != null) {
      // Maintenance lines publish under a line tag so `latest` never moves backward.
      args.push(`--tag=${distTag}`, '--staged');
    } else {
      args.push('--staged');
    }
    return { cmd: 'yarn', args, path: dir };
  },
};

export const cratesRegistry: Registry = {
  type: 'cargo',

  url(name, version) {
    return `https://crates.io/crates/${name}/${version}`;
  },

  // Reads the sparse index (not rate-limited).
  async exists(name, version) {
    const url = `https://index.crates.io/${crateIndexPath(name)}`;
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) });
      if (res.status === 404) {
        // The crate has never been published at all.
        return false;
      }
      if (!res.ok) {
        return null;
      }
      const lines = (await res.text()).split('\n').filter(line => line.length > 0);
      // Yanked versions still occupy their version number (it can never be re-published), so any
      // matching line counts as existing.
      return lines.some(line => {
        try {
          return (JSON.parse(line) as { vers?: string }).vers === version;
        } catch {
          return false;
        }
      });
    } catch {
      return null;
    }
  },

  isDuplicateRejection(output) {
    // crates.io: "crate version `x.y.z` is already uploaded"
    return /is already uploaded/i.test(output);
  },

  publishCommand({ name }) {
    return { cmd: 'cargo', args: ['publish', '--allow-dirty', '-p', name], path: '' };
  },
};

const registries: Record<RegistryType, Registry> = {
  npm: npmRegistry,
  cargo: cratesRegistry,
};

export function registryOf(type: RegistryType): Registry {
  return registries[type];
}

/** The registry a manifest type publishes to. */
export function registryOfManifest(type: 'package.json' | 'Cargo.toml'): Registry {
  return type === 'package.json' ? npmRegistry : cratesRegistry;
}

/** Sparse-index path for a crate name: `1/a`, `2/ab`, `3/a/abc`, `ab/cd/abcdefg`. */
export function crateIndexPath(name: string): string {
  const lower = name.toLowerCase();
  switch (lower.length) {
    case 1:
      return `1/${lower}`;
    case 2:
      return `2/${lower}`;
    case 3:
      return `3/${lower[0]}/${lower}`;
    default:
      return `${lower.slice(0, 2)}/${lower.slice(2, 4)}/${lower}`;
  }
}
