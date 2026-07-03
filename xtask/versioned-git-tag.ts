import type { Repository } from 'es-git';
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

  /**
   * Whether the tag ref exists, annotated or lightweight. Matches on the ref name because a
   * lightweight tag's oid is the commit itself, which `repo.findTag` cannot resolve — and GitHub
   * creates lightweight tags when a release's tag is missing.
   */
  exists(repo: Repository): boolean {
    let found = false;
    repo.tagForeach((_oid, name) => {
      if (name === this.tagRef) {
        found = true;
        return false;
      }
      return true;
    });
    return found;
  }
}
