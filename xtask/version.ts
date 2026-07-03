import { parse, type SemVer } from 'semver';

/**
 * How a stable version is bumped. Derived from a human choice during `prepare-release`
 * (conventional-commit/scope inference was dropped — contributors are not constrained), so
 * there is no longer a `prerelease` bump rule; prereleases go through {@link Version.toPrerelease}.
 */
export type BumpRule = 'major' | 'minor' | 'patch';

export interface Prerelease {
  /** The channel identifier, used as the npm dist-tag (e.g. `next`). */
  id: string;
  /** The build identifier appended after the id (e.g. a short commit hash, or a run number). */
  build: string;
}

export class Version {
  private ver: SemVer;

  static parse(raw: string): Version {
    const ver = parse(raw);
    if (ver == null) {
      throw new Error(`invalid version: ${raw}`);
    }
    return new Version(ver);
  }

  constructor(ver: SemVer) {
    this.ver = ver;
  }

  get prerelease(): Prerelease | null {
    if (this.ver.prerelease.length < 2) {
      return null;
    }
    return { id: String(this.ver.prerelease[0]), build: String(this.ver.prerelease[1]) };
  }

  equals(other: Version): boolean {
    return this.ver.compare(other.ver) === 0;
  }

  greaterThan(other: Version): boolean {
    return this.ver.compare(other.ver) > 0;
  }

  clone(): Version {
    return Version.parse(this.ver.toString());
  }

  bump(rule: BumpRule): this {
    this.ver = this.ver.inc(rule);
    return this;
  }

  /** Turn this into a prerelease of the *current* `x.y.z` base, e.g. `x.y.z-next.<build>`. */
  toPrerelease(id: string, build: string): this {
    const raw = `${this.ver.major}.${this.ver.minor}.${this.ver.patch}-${id}.${build}`;
    const parsed = parse(raw);
    if (parsed == null) {
      throw new Error(`invalid prerelease version: ${raw}`);
    }
    this.ver = parsed;
    return this;
  }

  toString(): string {
    return this.ver.toString();
  }
}
