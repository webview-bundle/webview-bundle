import fs from 'node:fs/promises';
import path from 'node:path';
import { glob } from 'tinyglobby';
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
import { registryOfManifest } from './registry.ts';
import { type BumpRule, Version } from './version.ts';

export const VersionedFileTypeSchema = z.enum(['package.json', 'Cargo.toml']);
export type VersionedFileType = z.infer<typeof VersionedFileTypeSchema>;

export interface VersionedFileRegistry {
  name: string;
  type: string;
  version: string;
  url: string;
}

export class VersionedFile {
  readonly type: VersionedFileType;
  private _nextVersion: Version | null;
  private pkgManager: PackageManager;

  static async loadAll(dir: string): Promise<VersionedFile[]> {
    // Glob patterns must use POSIX separators; `path.join` emits `\` on Windows, which globbers
    // treat as an escape (matching nothing), so build the patterns with forward slashes.
    const base = dir.replaceAll('\\', '/');
    const files = await glob(
      VersionedFileTypeSchema.options.map(fileType => path.posix.join(base, '**', fileType)),
      {
        cwd: ROOT_DIR,
        onlyFiles: true,
        ignore: ['**/node_modules/**', '**/target', '**/dist'],
      }
    );
    const versionedFiles = await Promise.all(files.map(x => VersionedFile.load(x)));
    return versionedFiles.filter((x): x is VersionedFile => x != null);
  }

  static async load(filepath: string): Promise<VersionedFile | null> {
    const absolutePath = path.join(ROOT_DIR, filepath);
    const filename = path.basename(absolutePath);
    const content = await fs.readFile(absolutePath, 'utf8');
    switch (filename) {
      case 'package.json':
      case 'Cargo.toml':
        return VersionedFile.parse(filename, filepath, content);
      default:
        throw new Error(`unrecognized file: ${filepath}`);
    }
  }

  /** Build a versioned file from raw manifest content (`null` for a `package.json` without a version). */
  static parse(type: VersionedFileType, filepath: string, content: string): VersionedFile | null {
    switch (type) {
      case 'package.json': {
        const pkg = PackageJson.create(filepath, content);
        return pkg == null ? null : new VersionedFile('package.json', pkg);
      }
      case 'Cargo.toml':
        return new VersionedFile('Cargo.toml', new Cargo(filepath, content));
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

  /** Names of the dependencies declared in this manifest (used to build the dependency graph). */
  get dependencyNames(): string[] {
    return this.pkgManager.dependencyNames;
  }

  get registry(): VersionedFileRegistry {
    const registry = registryOfManifest(this.type);
    const version = this.nextVersion.toString();
    return {
      name: this.name,
      type: registry.type,
      version,
      url: registry.url(this.name, version),
    };
  }

  bumpVersion(rule: BumpRule): void {
    this._nextVersion = this.pkgManager.version.clone();
    this._nextVersion.bump(rule);
  }

  /** Set the pending version to a prerelease of the current version (`x.y.z-<id>.<build>`). */
  setPrerelease(id: string, build: string): void {
    this._nextVersion = this.pkgManager.version.clone();
    this._nextVersion.toPrerelease(id, build);
  }

  write(): Action[] {
    if (!this.hasChanged) {
      return [];
    }
    return this.pkgManager.write(this.nextVersion);
  }

  /** The action publishing `version` of this manifest to its registry. */
  publishAction(version: Version, distTag?: string): Action {
    const registry = registryOfManifest(this.type);
    const command = registry.publishCommand({
      name: this.name,
      dir: path.dirname(this.path),
      version,
      distTag,
    });
    return {
      type: 'publish',
      registry: registry.type,
      manifest: this.name,
      version: version.toString(),
      cmd: command.cmd,
      args: command.args,
      path: command.path,
    };
  }
}

interface PackageManager {
  readonly name: string;
  readonly path: string;
  readonly version: Version;
  readonly canPublish: boolean;
  readonly dependencyNames: string[];
  write(nextVersion: Version): Action[];
}

class PackageJson implements PackageManager {
  private readonly json: PackageJsonType;
  private readonly _path: string;
  private readonly raw: string;

  static create(path: string, raw: string): PackageJson | null {
    const parsed: PackageJsonType = JSON.parse(raw);
    if (parsed.version == null) {
      return null;
    }
    if (parsed.name == null) {
      throw new Error('"name" field is required in package.json');
    }
    return new PackageJson(path, parsed, raw);
  }

  private constructor(path: string, json: PackageJsonType, raw: string) {
    this.json = json;
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

  get dependencyNames(): string[] {
    return [
      ...Object.keys(this.json.dependencies ?? {}),
      ...Object.keys(this.json.devDependencies ?? {}),
      ...Object.keys(this.json.peerDependencies ?? {}),
      ...Object.keys(this.json.optionalDependencies ?? {}),
    ];
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

  get dependencyNames(): string[] {
    const fromTable = (table?: Record<string, string | { package?: string }>): string[] =>
      Object.entries(table ?? {}).map(([key, value]) =>
        typeof value === 'object' && value.package != null ? value.package : key
      );
    return [
      ...fromTable(this.toml.dependencies),
      ...fromTable(this.toml['dev-dependencies']),
      ...fromTable(this.toml['build-dependencies']),
    ];
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
}
