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

      const pkgPath = path.relative(ROOT_DIR, path.dirname(configFile));
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

  get scopes(): readonly string[] {
    const scopes = this.config.scopes ?? [];
    return uniq([...scopes, this.name, 'all']);
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

  write(): Action[] {
    return this.versionedFiles.flatMap(x => x.write());
  }

  publish(): Action[] {
    return this.versionedFiles.flatMap(x => x.publish());
  }
}
