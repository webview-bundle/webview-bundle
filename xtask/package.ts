import fs from 'node:fs/promises';
import path from 'node:path';
import { uniq } from 'es-toolkit';
import { glob } from 'tinyglobby';
import type { Action } from './action.ts';
import {
  type Artifact,
  CONFIG_FILE,
  loadXtaskConfig,
  type PackageConfig,
  type Script,
} from './config.ts';
import { ROOT_DIR } from './consts.ts';
import type { BumpRule, Version } from './version.ts';
import { VersionedFile, VersionedFileTypeSchema } from './versioned-file.ts';
import { VersionedGitTag } from './versioned-git-tag.ts';

type NonEmptyArray<T> = readonly [T, ...T[]];

function isNonEmptyArray<T>(x: readonly T[]): x is NonEmptyArray<T> {
  return x.length > 0;
}

/** Whether `dir` itself (not a subdirectory) holds a versioned manifest. */
async function hasDirectManifest(dir: string): Promise<boolean> {
  const checks = await Promise.all(
    VersionedFileTypeSchema.options.map(async fileType => {
      try {
        await fs.access(path.join(ROOT_DIR, dir, fileType));
        return true;
      } catch {
        return false;
      }
    })
  );
  return checks.some(Boolean);
}

export class Package {
  public readonly name: string;
  public readonly path: string;
  public readonly versionedFiles: NonEmptyArray<VersionedFile>;
  private readonly config: PackageConfig;

  static async loadAll(): Promise<Package[]> {
    const config = await loadXtaskConfig();

    // Resolve entries to package dirs. Glob entries only discover dirs (with the default config);
    // object entries always win, so a dir both matched by a glob and configured explicitly gets
    // the explicit config regardless of entry order.
    const dirs = new Map<string, PackageConfig>();
    for (const entry of config.packages) {
      if (typeof entry === 'string') {
        // Globs (and the paths derived from them) are POSIX: `pkg.path` feeds glob patterns and
        // git pathspecs, both of which require forward slashes.
        const matched = await glob(entry, {
          cwd: ROOT_DIR,
          onlyDirectories: true,
          ignore: ['**/node_modules/**', '**/target', '**/dist'],
        });
        for (const dir of matched) {
          const pkgPath = dir.replace(/\/+$/, '');
          // A dir without a manifest of its own is not a package (e.g. `packages/remote` only
          // groups packages); skip it instead of absorbing its children.
          if (!(await hasDirectManifest(pkgPath))) {
            continue;
          }
          if (!dirs.has(pkgPath)) {
            dirs.set(pkgPath, {});
          }
        }
      } else {
        const { path: pkgPath, ...pkgConfig } = entry;
        if (!(await hasDirectManifest(pkgPath))) {
          throw new Error(`No manifest found in "${pkgPath}" (from "${CONFIG_FILE}")`);
        }
        dirs.set(pkgPath, pkgConfig);
      }
    }

    const packages: Package[] = [];
    for (const [pkgPath, pkgConfig] of [...dirs.entries()].sort(([a], [b]) => a.localeCompare(b))) {
      const pkgName = pkgConfig.name ?? path.posix.basename(pkgPath);

      const versionedFiles = await VersionedFile.loadAll(pkgPath);
      if (!isNonEmptyArray(versionedFiles)) {
        throw new Error(`Cannot load versioned files from "${pkgPath}"`);
      }

      packages.push(new Package(pkgName, pkgPath, versionedFiles, pkgConfig));
    }

    return packages;
  }

  static buildGraph(packages: Package[]): PackageGraph {
    const provideToPkg = new Map<string, Package>();
    for (const pkg of packages) {
      for (const name of pkg.versionFileNames) {
        provideToPkg.set(name, pkg);
      }
    }
    const dependencies = new Map<Package, Set<Package>>();
    const dependents = new Map<Package, Set<Package>>();
    for (const pkg of packages) {
      dependencies.set(pkg, new Set());
      dependents.set(pkg, new Set());
    }
    for (const pkg of packages) {
      for (const depName of pkg.versionFileDependencyNames) {
        const dep = provideToPkg.get(depName);
        if (dep != null && dep !== pkg) {
          dependencies.get(pkg)!.add(dep);
          dependents.get(dep)!.add(pkg);
        }
      }
    }
    return {
      packages,
      dependenciesOf: pkg => [...(dependencies.get(pkg) ?? [])],
      dependentsOf: pkg => [...(dependents.get(pkg) ?? [])],
      transitiveDependents(seeds) {
        const result = new Set<Package>();
        const stack = [...seeds];
        while (stack.length > 0) {
          const current = stack.pop()!;
          for (const dependent of dependents.get(current) ?? []) {
            if (!result.has(dependent)) {
              result.add(dependent);
              stack.push(dependent);
            }
          }
        }
        return result;
      },
      topoSort(subset) {
        const set = new Set(subset);
        const visited = new Set<Package>();
        const order: Package[] = [];
        const visit = (pkg: Package): void => {
          if (visited.has(pkg)) {
            return;
          }
          visited.add(pkg);
          for (const dep of dependencies.get(pkg) ?? []) {
            if (set.has(dep)) {
              visit(dep);
            }
          }
          order.push(pkg);
        };
        for (const pkg of set) {
          visit(pkg);
        }
        return order;
      },
    };
  }

  constructor(
    name: string,
    path: string,
    versionedFiles: NonEmptyArray<VersionedFile>,
    config: PackageConfig
  ) {
    this.name = name;
    this.path = path;
    this.versionedFiles = versionedFiles;
    this.config = config;
  }

  get absolutePath(): string {
    return path.join(ROOT_DIR, this.path);
  }

  get changelog(): string {
    return this.config.changelog ?? path.join(this.path, 'CHANGELOG.md');
  }

  /** The package names this package provides across all its manifests (npm names and/or crate names). */
  get versionFileNames(): string[] {
    return uniq(this.versionedFiles.map(f => f.name));
  }

  /** The dependency names declared across all its manifests (used to build the dependency graph). */
  get versionFileDependencyNames(): string[] {
    return uniq(this.versionedFiles.flatMap(f => f.dependencyNames));
  }

  /** Whether any of this package's manifests can be published (e.g. not `private`/`publish = false`). */
  get canPublish(): boolean {
    return this.versionedFiles.some(f => f.canPublish);
  }

  get artifacts(): readonly Artifact[] {
    return this.config.artifacts ?? [];
  }

  get beforePublishScripts(): readonly Script[] {
    return this.config.beforePublishScripts ?? [];
  }

  get assets(): readonly string[] {
    return this.config.assets ?? [];
  }

  get version(): Version {
    return this.versionedFiles[0].version;
  }

  get nextVersion(): Version {
    return this.versionedFiles[0].nextVersion;
  }

  get hasChanged(): boolean {
    return this.versionedFiles[0].hasChanged;
  }

  get versionedGitTag(): VersionedGitTag {
    return new VersionedGitTag(this.name, this.version);
  }

  get nextVersionedGitTag(): VersionedGitTag {
    return new VersionedGitTag(this.name, this.nextVersion);
  }

  bumpVersion(rule: BumpRule): this {
    for (const versionedFile of this.versionedFiles) {
      versionedFile.bumpVersion(rule);
    }
    return this;
  }

  /**
   * Why `version` cannot be this package's next version, or `null` when it can. It has to be ahead
   * of *every* manifest: `release` only publishes what the merge commit raised, and a package's
   * nested manifests (e.g. the napi platform packages) must not be walked backwards.
   */
  rejectVersion(version: Version): string | null {
    const blocking = this.versionedFiles.find(file => !version.greaterThan(file.version));
    return blocking == null
      ? null
      : `${blocking.name} is already at ${blocking.version.toString()}`;
  }

  /**
   * Set every manifest's pending version to `version`, instead of deriving it from a bump rule.
   * Unlike {@link bumpVersion} — which advances each manifest from its own version — this lands
   * them all on one version, so a package's manifests can be aligned with each other (and with
   * other packages).
   */
  setVersion(version: Version): this {
    const reason = this.rejectVersion(version);
    if (reason != null) {
      throw new Error(`cannot set ${this.name} to ${version.toString()}: ${reason}`);
    }
    for (const versionedFile of this.versionedFiles) {
      versionedFile.setVersion(version);
    }
    return this;
  }

  /** Set every manifest's pending version to a prerelease of its current version. */
  bumpPrerelease(id: string, build: string): this {
    for (const versionedFile of this.versionedFiles) {
      versionedFile.setPrerelease(id, build);
    }
    return this;
  }

  write(): Action[] {
    return this.versionedFiles.flatMap(x => x.write());
  }
}

/**
 * The dependency graph over packages, derived from `package.json`/`Cargo.toml` dependency names
 * (an edge `A -> B` means a manifest of `A` depends on a package name `B` provides). It replaces
 * conventional-commit scopes for deciding propagation: when a package changes, every package that
 * (transitively) depends on it must be released too so it picks up the new version.
 */
export interface PackageGraph {
  readonly packages: readonly Package[];
  /** Packages that `pkg` directly depends on. */
  dependenciesOf(pkg: Package): Package[];
  /** Packages that directly depend on `pkg`. */
  dependentsOf(pkg: Package): Package[];
  /** All packages that transitively depend on any of `seeds` (excluding the seeds themselves). */
  transitiveDependents(seeds: Iterable<Package>): Set<Package>;
  /** Order `subset` so that dependencies come before the packages that depend on them. */
  topoSort(subset: Iterable<Package>): Package[];
}
