import type { Commit, Repository, Tree } from 'es-git';
import { RELEASE_COMMIT_PREFIX } from './consts.ts';

/**
 * Files that don't constitute a releasable change on their own. Markdown docs (README, CHANGELOG,
 * …) are written as part of releasing or are docs-only edits, so a commit touching only these
 * isn't attributed to a package.
 */
function isIgnoredChangePath(file: string): boolean {
  const lower = file.toLowerCase();
  return lower.endsWith('.md') || lower.endsWith('.markdown');
}

/**
 * Whether `commit` changed any non-ignored file matching `pathspecs`, by diffing its tree against
 * its first parent (an empty tree for the root commit). Path-based attribution replaces
 * conventional-commit scopes for deciding which package a commit belongs to.
 */
function changedUnderPaths(repo: Repository, commit: Commit, pathspecs: string[]): boolean {
  const tree = commit.tree();
  let parentTree: Tree | undefined;
  try {
    // `<oid>^` resolves to the first parent; throws for the root commit (no parent).
    parentTree = repo.getCommit(repo.revparseSingle(`${commit.id()}^`)).tree();
  } catch {
    parentTree = undefined;
  }
  const deltas = repo.diffTreeToTree(parentTree, tree, { pathspecs }).deltas();
  for (let next = deltas.next(); next.done !== true; next = deltas.next()) {
    const file = next.value.newFile().path() ?? next.value.oldFile().path();
    if (file != null && !isIgnoredChangePath(file)) {
      return true;
    }
  }
  return false;
}

/**
 * Commits that touched any of `paths`, excluding release commits (the version-bump commits
 * `prepare-release` makes) and commits that only changed ignored files (markdown docs).
 *
 * When `since` is a ref (a package's last release tag) the range is `since..HEAD`; when it is
 * `null` (the package was never released) the whole history reachable from `HEAD` is scanned.
 */
export function commitsTouchingPaths(
  repo: Repository,
  since: string | null,
  paths: string[]
): Commit[] {
  if (paths.length === 0) {
    return [];
  }
  const revwalk = repo.revwalk();
  if (since != null) {
    revwalk.pushRange(`${since}..HEAD`);
  } else {
    revwalk.pushHead();
  }
  const commits: Commit[] = [];
  for (let oid = revwalk.next(); oid != null; oid = revwalk.next()) {
    const commit = repo.getCommit(oid);
    // A release commit is bookkeeping, not a change to be released again.
    if ((commit.summary() ?? '').startsWith(RELEASE_COMMIT_PREFIX)) {
      continue;
    }
    if (changedUnderPaths(repo, commit, paths)) {
      commits.push(commit);
    }
  }
  return commits;
}
