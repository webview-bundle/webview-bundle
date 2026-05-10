import type { Commit, Repository } from 'es-git';
import { isNotNil } from 'es-toolkit';
import { ConventionalCommit } from './conventional-commit.ts';
import type { BumpRule } from './version.ts';
import type { VersionedGitTag } from './versioned-git-tag.ts';

export class Change {
  public readonly commit: ConventionalCommit;

  static tryFromCommit(commit: Commit): Change {
    return new Change(ConventionalCommit.parseCommit(commit));
  }

  constructor(commit: ConventionalCommit) {
    this.commit = commit;
  }

  isInScopes(scopes: readonly string[]): boolean {
    if (this.commit.scopes.length === 0) {
      return false;
    }
    return this.commit.scopes.some(x => scopes.includes(x));
  }

  toString() {
    const { type, scopes, isBreaking, summary } = this.commit;
    const prefix = isBreaking ? '[BREAKING CHANGE] ' : '';
    const scopesStr = scopes.length > 0 ? `(${scopes.join(',')})` : '';
    return `${prefix}${type}${scopesStr}: ${summary}`;
  }
}

export class Changes {
  public readonly changes: ReadonlyArray<Change>;

  static tryFromGitTag(
    repo: Repository,
    prevTag: VersionedGitTag,
    scopes: readonly string[]
  ): Changes {
    const head = repo.head().target();
    if (head == null) {
      throw new Error('cannot find git `HEAD` target');
    }
    const tag = prevTag.findTag(repo);
    const revwalk = repo.revwalk();
    revwalk.push(head);
    if (tag != null) {
      revwalk.hide(tag.id());
    }
    const changes = Changes.getChangesFromCommits(repo, [...revwalk]).filter(x =>
      x.isInScopes(scopes)
    );
    return new Changes(changes);
  }

  static fromCommits(repo: Repository, commits: string[]): Changes {
    return new Changes(Changes.getChangesFromCommits(repo, commits));
  }

  private static getChangesFromCommits(repo: Repository, commits: string[]): Change[] {
    const changes = commits
      .map(oid => repo.findCommit(oid))
      .filter(isNotNil)
      .map(commit => {
        try {
          return Change.tryFromCommit(commit);
        } catch {
          return null;
        }
      })
      .filter(isNotNil);
    return changes;
  }

  constructor(changes: Change[]) {
    this.changes = changes;
  }

  getBumpRule(): Extract<BumpRule, { type: 'major' | 'minor' | 'patch' }> | null {
    if (this.changes.length === 0) {
      return null;
    }
    if (this.changes.some(x => x.commit.isBreaking)) {
      return { type: 'major' };
    }
    if (this.changes.some(x => x.commit.type === 'feat')) {
      return { type: 'minor' };
    }
    return { type: 'patch' };
  }
}
