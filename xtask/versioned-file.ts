import fs from 'node:fs/promises';
import path from 'node:path';
import glob from 'fast-glob';
import type { PackageJson as PackageJsonType } from 'type-fest';
import { z } from 'zod';
import type { Action } from './action.ts';
import {
  type CargoToml,
  editCargoTomlVersion,
  formatCargoToml,
  parseCargoToml,
} from './cargo-toml.ts';
import { ROOT_DIR } from './consts.ts';
import { type BumpRule, Version } from './version.ts';

export const VersionedFileTypeSchema = z.enum(['package.json', 'Cargo.toml']);
export type VersionedFileType = z.infer<typeof VersionedFileTypeSchema>;

export class VersionedFile {
  readonly type: VersionedFileType;
  private _nextVersion: Version | null;
  private pkgManager: PackageManager;

  static async loadAll(dir: string): Promise<VersionedFile[]> {
    const files = await glob(
      VersionedFileTypeSchema.options.map(fileType => path.join(dir, '**', fileType)),
      {
        cwd: ROOT_DIR,
        onlyFiles: true,
        ignore: ['**/node_modules/**', '**/target', '**/dist'],
      }
    );
    const versionedFiles = await Promise.all(files.map(x => VersionedFile.load(x)));
    return versionedFiles;
  }

  static async load(filepath: string): Promise<VersionedFile> {
    const absolutePath = path.join(ROOT_DIR, filepath);
    const filename = path.basename(absolutePath);
    const content = await fs.readFile(absolutePath, 'utf8');
    switch (filename) {
      case 'package.json':
        return new VersionedFile('package.json', new PackageJson(filepath, content));
      case 'Cargo.toml':
        return new VersionedFile('Cargo.toml', new Cargo(filepath, content));
      default:
        throw new Error(`unrecognized file: ${filepath}`);
    }
  }

  constructor(type: VersionedFileType, pkgManager: PackageManager) {
    this.type = type;
    this._nextVersion = null;
    this.pkgManager = pkgManager;
  }

  get name(): string {
    return this.pkgManager.name;
  }

  get path(): string {
    return this.pkgManager.path;
  }

  get version(): Version {
    return this.pkgManager.version.clone();
  }

  get nextVersion(): Version {
    if (this._nextVersion != null) {
      return this._nextVersion.clone();
    }
    return this.pkgManager.version.clone();
  }

  get hasChanged(): boolean {
    if (this._nextVersion == null) {
      return false;
    }
    return !this.pkgManager.version.equals(this._nextVersion);
  }

  get canPublish(): boolean {
    return this.pkgManager.canPublish;
  }

  bumpVersion(rule: BumpRule): void {
    this._nextVersion = this.pkgManager.version.clone();
    this._nextVersion.bump(rule);
  }

  write(): Action[] {
    if (!this.hasChanged) {
      return [];
    }
    return this.pkgManager.write(this.nextVersion);
  }

  publish(): Action[] {
    if (!this.hasChanged || !this.canPublish) {
      return [];
    }
    return this.pkgManager.publish(this.nextVersion);
  }
}

interface PackageManager {
  readonly name: string;
  readonly path: string;
  readonly version: Version;
  readonly canPublish: boolean;
  write(nextVersion: Version): Action[];
  publish(nextVersion: Version): Action[];
}

class PackageJson implements PackageManager {
  private readonly json: PackageJsonType;
  private readonly _path: string;
  private readonly raw: string;

  constructor(path: string, raw: string) {
    const parsed: PackageJsonType = JSON.parse(raw);
    if (parsed.name == null) {
      throw new Error('"name" field is required in package.json');
    }
    if (parsed.version == null) {
      throw new Error('"version" field is required in package.json');
    }
    this.json = parsed;
    this._path = path;
    this.raw = raw;
  }

  get name(): string {
    return this.json.name!;
  }

  get path(): string {
    return this._path;
  }

  get version(): Version {
    return Version.parse(this.json.version!);
  }

  get canPublish(): boolean {
    return this.json.private !== true;
  }

  write(nextVersion: Version): Action[] {
    const json = { ...this.json };
    json.version = nextVersion.toString();

    const content = `${JSON.stringify(json, null, 2)}\n`;
    return [
      {
        type: 'write',
        path: this.path,
        content,
        prevContent: this.raw,
      },
    ];
  }

  publish(nextVersion: Version): Action[] {
    const args = ['npm', 'publish', '--access=public', '--provenance'];
    const prerelease = nextVersion.prerelease;
    if (prerelease != null) {
      args.push(`--tag=${prerelease.id}`);
    }
    return [
      {
        type: 'command',
        cmd: 'yarn',
        args,
        path: path.dirname(this.path),
      },
    ];
  }
}

class Cargo implements PackageManager {
  private readonly toml: CargoToml;
  private readonly _path: string;
  private readonly raw: string;

  constructor(_path: string, raw: string) {
    const parsed = parseCargoToml(raw);
    if (parsed.package?.name == null) {
      throw new Error('"name" field is required in Cargo.toml');
    }
    if (parsed.package?.version == null) {
      throw new Error('"version" field is required in Cargo.toml');
    }
    this.toml = parsed;
    this._path = _path;
    this.raw = raw;
  }

  get name(): string {
    return this.toml.package!.name!;
  }

  get path(): string {
    return this._path;
  }

  get version(): Version {
    return Version.parse(this.toml.package!.version!);
  }

  get canPublish(): boolean {
    return this.toml.package?.publish !== false;
  }

  write(nextVersion: Version): Action[] {
    const edited = parseCargoToml(this.raw);
    editCargoTomlVersion(edited, nextVersion);
    const content = formatCargoToml(edited);

    return [
      {
        type: 'write',
        path: this.path,
        content,
        prevContent: this.raw,
      },
    ];
  }

  publish(_nextVersion: Version): Action[] {
    return [
      {
        type: 'command',
        cmd: 'cargo',
        args: ['publish', '--allow-dirty', '-p', this.name],
        path: '',
      },
    ];
  }
}
