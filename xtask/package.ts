import path from 'node:path';
import { uniq } from 'es-toolkit';
import type { Action } from './action.ts';
import type { Config, PackageConfig, ScriptConfig } from './config.ts';
import type { BumpRule, Version } from './version.ts';
import { VersionedFile } from './versioned-file.ts';
import { VersionedGitTag } from './versioned-git-tag.ts';

type NonEmptyArray<T> = readonly [T, ...T[]];

function isNonEmptyArray<T>(x: readonly T[]): x is NonEmptyArray<T> {
  return x.length > 0;
}

export class Package {
  public readonly name: string;
  public readonly versionedFiles: NonEmptyArray<VersionedFile>;
  private readonly config: PackageConfig;

  static async loadAll(config: Config): Promise<Package[]> {
    const packages: Package[] = [];
    for (const [name, pkgConfig] of Object.entries(config.packages)) {
      const versionedFiles = await VersionedFile.loadAll(pkgConfig.path);
      if (!isNonEmptyArray(versionedFiles)) {
        throw new Error(`Cannot load versioned files from "${name}"`);
      }
      packages.push(new Package(name, versionedFiles, pkgConfig));
    }
    return packages;
  }

  constructor(name: string, versionedFiles: NonEmptyArray<VersionedFile>, config: PackageConfig) {
    this.name = name;
    this.versionedFiles = versionedFiles;
    this.config = config;
  }

  get changelog(): string {
    return this.config.changelog ?? path.join(this.config.path, 'CHANGELOG.md');
  }

  get scopes(): readonly string[] {
    const scopes = this.config.scopes ?? [];
    return uniq([...scopes, this.name, 'all']);
  }

  get beforePublishScripts(): ScriptConfig[] {
    return this.config.beforePublishScripts ?? [];
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
