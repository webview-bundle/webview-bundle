import path from 'node:path';
import { uniq } from 'es-toolkit';
import { glob } from 'tinyglobby';
import type { Action } from './action.ts';
import { ROOT_DIR } from './consts.ts';
import {
  type Artifact,
  loadPackageConfig,
  type PackageConfig,
  type Script,
} from './package-config.ts';
import type { BumpRule, Version } from './version.ts';
import { VersionedFile } from './versioned-file.ts';
import { VersionedGitTag } from './versioned-git-tag.ts';

type NonEmptyArray<T> = readonly [T, ...T[]];

function isNonEmptyArray<T>(x: readonly T[]): x is NonEmptyArray<T> {
  return x.length > 0;
}

export class Package {
  public readonly name: string;
  public readonly path: string;
  public readonly versionedFiles: NonEmptyArray<VersionedFile>;
  private readonly config: PackageConfig;

  static async loadAll(): Promise<Package[]> {
    const packages: Package[] = [];
    const configFiles = await glob('packages/**/xtask.config.json', {
      cwd: ROOT_DIR,
      onlyFiles: true,
    });

    for (const configFile of configFiles) {
      const config = await loadPackageConfig(path.join(ROOT_DIR, configFile));

      // Normalize to POSIX separators: `pkg.path` feeds glob patterns and git pathspecs, both of
      // which require forward slashes (`path.relative` yields `\` on Windows).
      const pkgPath = path.relative(ROOT_DIR, path.dirname(configFile)).replaceAll('\\', '/');
      const dirName = path.basename(path.dirname(configFile));
      const pkgName = config.name ?? dirName;

      const versionedFiles = await VersionedFile.loadAll(pkgPath);
      if (!isNonEmptyArray(versionedFiles)) {
        throw new Error(`Cannot load versioned files from "${pkgPath}"`);
      }

      packages.push(new Package(pkgName, pkgPath, versionedFiles, config));
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

  publish(distTag?: string): Action[] {
    return this.versionedFiles.flatMap(x => x.publish(distTag));
  }

  /** Publish actions for the current (already-written) version. Used by tag-based publish. */
  publishCurrent(distTag?: string): Action[] {
    return this.versionedFiles.flatMap(x => x.publishCurrent(distTag));
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
