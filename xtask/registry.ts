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

export type RegistryType = 'npm' | 'cargo' | 'jsr';

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
/** Extra attempts after the first, for the registry existence checks. */
const FETCH_RETRIES = 2;
const RETRY_BASE_MS = 500;

export interface FetchRetryOptions {
  retries?: number;
  baseDelayMs?: number;
  timeoutMs?: number;
}

const sleep = (ms: number): Promise<void> => new Promise(resolve => setTimeout(resolve, ms));

/**
 * `fetch` with retry on *transient* failures — a thrown network/timeout error, or a retryable
 * status (429, or >= 500) — using exponential backoff. A definitive answer (any other status,
 * including 200/404) returns immediately without a retry. After the last attempt the final
 * response is returned (or the last error re-thrown) so a caller can fall back to `null`
 * (unknown). Without this a single network blip during a release would skip the
 * already-published check and trigger an unnecessary publish attempt.
 */
export async function fetchWithRetry(url: string, opts: FetchRetryOptions = {}): Promise<Response> {
  const {
    retries = FETCH_RETRIES,
    baseDelayMs = RETRY_BASE_MS,
    timeoutMs = FETCH_TIMEOUT_MS,
  } = opts;
  let attempt = 0;
  while (true) {
    try {
      const res = await fetch(url, { signal: AbortSignal.timeout(timeoutMs) });
      if (attempt >= retries || (res.status < 500 && res.status !== 429)) {
        return res;
      }
    } catch (e) {
      if (attempt >= retries) {
        throw e;
      }
    }
    attempt += 1;
    if (baseDelayMs > 0) {
      await sleep(baseDelayMs * 2 ** (attempt - 1));
    }
  }
}

export const npmRegistry: Registry = {
  type: 'npm',

  url(name, version) {
    return `https://www.npmjs.com/package/${name}/v/${version}`;
  },

  async exists(name, version) {
    // Scoped names keep the `@` but encode the `/` (`@scope%2Fname`).
    const url = `https://registry.npmjs.org/${name.replace('/', '%2F')}/${version}`;
    try {
      const res = await fetchWithRetry(url);
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
      const res = await fetchWithRetry(url);
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

export const jsrRegistry: Registry = {
  type: 'jsr',

  url(name, version) {
    return `https://jsr.io/${name}@${version}`;
  },

  // Queries the same JSR API endpoint `deno publish`'s own pre-flight check uses. A yanked version
  // still returns 200 — correct here, since its version number can never be re-published.
  async exists(name, version) {
    const scoped = /^@([^/]+)\/(.+)$/.exec(name);
    if (scoped == null) {
      return null;
    }
    const [, scope, pkg] = scoped;
    const url = `https://api.jsr.io/scopes/${scope}/packages/${pkg}/versions/${version}`;
    try {
      const res = await fetchWithRetry(url);
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

  isDuplicateRejection() {
    // `deno publish` is idempotent: re-publishing an existing version is a no-op that exits 0, so
    // a duplicate never reaches the non-zero path this hook classifies.
    return false;
  },

  publishCommand({ dir }) {
    // JSR has no dist-tags; prereleases are automatically excluded from `latest` and semver
    // ranges, so the channel is irrelevant. `--allow-dirty` because the release flow writes the
    // new version just before publishing.
    return { cmd: 'deno', args: ['publish', '--allow-dirty'], path: dir };
  },
};

const registries: Record<RegistryType, Registry> = {
  npm: npmRegistry,
  cargo: cratesRegistry,
  jsr: jsrRegistry,
};

export function registryOf(type: RegistryType): Registry {
  return registries[type];
}

/** The registry a manifest type publishes to. */
export function registryOfManifest(type: 'package.json' | 'Cargo.toml' | 'deno.json'): Registry {
  switch (type) {
    case 'package.json':
      return npmRegistry;
    case 'Cargo.toml':
      return cratesRegistry;
    case 'deno.json':
      return jsrRegistry;
  }
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
