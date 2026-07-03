/**
 * Existence checks against the public registries, used to skip already-published versions when a
 * failed `release`/`prerelease` run is retried. Every check returns `null` when it cannot be
 * answered (network error, unexpected status); the caller should then attempt the publish and let
 * the registry reject a duplicate.
 */

const FETCH_TIMEOUT_MS = 10_000;

/** Whether `name@version` is already published to the npm registry. */
export async function npmVersionExists(name: string, version: string): Promise<boolean | null> {
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
}

/** Whether `name@version` is already published to crates.io. Reads the sparse index (no rate limit). */
export async function cratesVersionExists(name: string, version: string): Promise<boolean | null> {
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
}

const ALREADY_PUBLISHED_REJECTIONS = [
  /cannot publish over/i, // npm E403: "You cannot publish over the previously published versions"
  /EPUBLISHCONFLICT/i,
  /is already uploaded/i, // crates.io: "crate version `x.y.z` is already uploaded"
];

/**
 * Whether a failed publish command's output is the registry rejecting an already-existing version.
 * Backstops the existence checks for versions they cannot see — notably npm's *staged* publishes
 * (stable channel), which reserve the version but stay hidden until approved.
 */
export function isAlreadyPublishedRejection(output: string | undefined): boolean {
  if (output == null) {
    return false;
  }
  return ALREADY_PUBLISHED_REJECTIONS.some(pattern => pattern.test(output));
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
