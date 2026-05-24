import type { Repository, Tag } from 'es-git';
import type { Version } from './version.ts';

export class VersionedGitTag {
  public readonly name: string;
  private readonly _version: Version;

  constructor(name: string, version: Version) {
    this.name = name;
    this._version = version;
  }

  get version(): Version {
    return this._version.clone();
  }

  get tagName(): string {
    return `${this.name}/${this._version.toString()}`;
  }

  get tagRef(): string {
    return `refs/tags/${this.tagName}`;
  }

  findTag(repo: Repository): Tag | null {
    let tagOid: string | undefined;
    repo.tagForeach((oid, name) => {
      if (name === this.tagRef) {
        tagOid = oid;
        return false;
      }
      return true;
    });
    if (tagOid == null) {
      return null;
    }
    return repo.findTag(tagOid);
  }

  exists(repo: Repository): boolean {
    return this.findTag(repo) != null;
  }
}
