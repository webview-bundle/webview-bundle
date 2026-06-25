import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { Commit, Repository, Tree } from 'es-git';
import { isNotNil, uniq } from 'es-toolkit';
import type { PackageJson } from 'type-fest';
import { runActions } from './action.ts';
import { editCargoTomlVersion, formatCargoToml, parseCargoToml } from './cargo-toml.ts';
import { Changelog } from './changelog.ts';
import type { Changes } from './changes.ts';
import { c } from './console.ts';
import { ROOT_DIR } from './consts.ts';
import { commitsTouchingPaths } from './git.ts';
import { Package, type PackageGraph } from './package.ts';
import type { VersionedFile, VersionedFileRegistry } from './versioned-file.ts';

export interface ReleaseTarget {
  package: Package;
  changes: Changes;
  changelog: Changelog | null;
}

/** The packages to release, in dependency order (dependencies first). */
export interface ReleasePlan {
  graph: PackageGraph;
  directChanges: Map<Package, Commit[]>;
  candidates: Package[];
}

/**
 * Resolve the release candidates: packages changed since their last tag, plus every package that
 * (transitively) depends on one of them. Includes packages that are not published to a registry
 * (`canPublish === false`) — they still get version bumps + changelog entries committed to git and
 * a git tag; only the registry push is skipped (per manifest). Ordered dependencies-first so a
 * dependent can read its dependency's already-decided next version. Shared by `prepare-release`
 * (interactive) and `prerelease` (automatic).
 */
export async function planRelease(repo: Repository): Promise<ReleasePlan> {
  const packages = await Package.loadAll();
  const graph = Package.buildGraph(packages);
  const directChanges = computeDirectChanges(repo, packages);
  const directChanged = packages.filter(pkg => (directChanges.get(pkg)?.length ?? 0) > 0);
  const propagated = [...graph.transitiveDependents(directChanged)].filter(
    pkg => !directChanged.includes(pkg)
  );
  const candidates = graph.topoSort([...directChanged, ...propagated]);
  return { graph, directChanges, candidates };
}

/**
 * For every package, the commits since its last release tag (`<name>/<version>`) that touched
 * files under the package directory. These are the package's "direct" changes; dependency-driven
 * releases are derived from the graph on top of this.
 */
export function computeDirectChanges(
  repo: Repository,
  packages: Package[]
): Map<Package, Commit[]> {
  const result = new Map<Package, Commit[]>();
  for (const pkg of packages) {
    const tag = pkg.versionedGitTag;
    const since = tag.exists(repo) ? tag.tagName : null;
    result.set(pkg, commitsTouchingPaths(repo, since, [pkg.path]));
  }
  return result;
}

/** The version recorded in a manifest at a given tree, or `null` if it is absent/unparseable. */
function versionAtTree(repo: Repository, tree: Tree, file: VersionedFile): string | null {
  const entry = tree.getPath(file.path);
  if (entry == null) {
    return null;
  }
  try {
    const content = new TextDecoder().decode(entry.toObject(repo).peelToBlob().content());
    switch (file.type) {
      case 'package.json':
        return (JSON.parse(content) as PackageJson)?.version ?? null;
      case 'Cargo.toml':
        return parseCargoToml(content).package?.version ?? null;
      case 'deno.json':
        return (JSON.parse(content) as { version?: string })?.version ?? null;
    }
  } catch {
    return null;
  }
}

/**
 * Packages whose version was changed by the `HEAD` commit, compared to its first parent — i.e. the
 * packages bumped by a merged `prepare-release` commit. A package counts only if one of its
 * manifests existed in the parent with a *different* version: a manifest that is absent from the
 * parent is a brand-new package (or a newly added manifest), not a bump, so the commit that first
 * introduces a package does not publish it. Such a package is released through the normal
 * `prepare-release` flow, whose merge commit bumps the (now-existing) manifest off its initial
 * version. This is how `release` decides what to publish, so an ordinary merge (no version change)
 * publishes nothing.
 */
export function packagesBumpedInHead(repo: Repository, packages: Package[]): Package[] {
  const head = repo.head().target();
  if (head == null) {
    return [];
  }
  let parentTree: Tree | null = null;
  try {
    // `<oid>^` is the first parent; throws for the root commit (then there is no prior version for
    // any manifest, so nothing is treated as a bump).
    parentTree = repo.getCommit(repo.revparseSingle(`${head}^`)).tree();
  } catch {
    parentTree = null;
  }
  return packages.filter(pkg =>
    pkg.versionedFiles.some(file => {
      const previous = parentTree != null ? versionAtTree(repo, parentTree, file) : null;
      // A manifest absent from the parent (`previous == null`) is new, not bumped — skip it so new
      // packages aren't published on the commit that adds them.
      return previous != null && previous !== file.version.toString();
    })
  );
}

/** Write version files, per-package changelogs, the root Cargo.toml, and the root CHANGELOG. */
export async function writeReleaseTargets(
  targets: ReleaseTarget[],
  opts: { dryRun?: boolean } = {}
): Promise<void> {
  const dryRun = opts.dryRun ?? false;
  for (const target of targets) {
    await runActions(target.package.write(), { name: target.package.name, dryRun });
    if (target.changelog != null) {
      target.changelog.appendChanges(target.package, target.changes);
      await runActions(target.changelog.write(), { name: target.package.name, dryRun });
    }
  }
  await writeRootCargoToml(targets, dryRun);
  await writeRootChangelog(targets, dryRun);
}

async function writeRootCargoToml(targets: ReleaseTarget[], dryRun: boolean): Promise<boolean> {
  const hasCargoChanged = targets
    .filter(x => x.package.hasChanged)
    .flatMap(x => x.package.versionedFiles)
    .some(x => x.type === 'Cargo.toml');
  if (!hasCargoChanged) {
    return false;
  }
  const raw = await fs.readFile(path.join(ROOT_DIR, 'Cargo.toml'), 'utf8');
  const toml = parseCargoToml(raw);
  for (const target of targets) {
    for (const versionedFile of target.package.versionedFiles) {
      if (versionedFile.type !== 'Cargo.toml') {
        continue;
      }
      editCargoTomlVersion(toml, versionedFile.nextVersion, versionedFile.name);
    }
  }
  await runActions(
    [{ type: 'write', path: 'Cargo.toml', content: formatCargoToml(toml), prevContent: raw }],
    { dryRun }
  );
  return true;
}

async function writeRootChangelog(targets: ReleaseTarget[], dryRun: boolean): Promise<boolean> {
  const changelog = await Changelog.load('CHANGELOG.md');
  for (const target of targets) {
    changelog.appendChanges(target.package, target.changes);
  }
  await runActions(changelog.write(), { dryRun });
  return true;
}

/**
 * A stable, content-addressed branch name for a set of release targets.
 *
 * Derived from the sorted `<name>@<nextVersion>` list, so the same set of bumps always maps to
 * the same `release/<hash>` branch (a refresh updates the same PR), while a different set of
 * packages/versions opens a separate one. This replaces a single fixed branch, which would
 * otherwise force unrelated release target sets to share (and clobber) one PR.
 */
export function releaseBranchName(targets: ReleaseTarget[]): string {
  const key = targets
    .map(target => `${target.package.name}@${target.package.nextVersion.toString()}`)
    .sort()
    .join('\n');
  const hash = crypto.createHash('sha256').update(key).digest('hex').slice(0, 12);
  return `release/${hash}`;
}

/** The file paths touched by a release (for staging into a git commit). */
export function releasePathspecs(targets: ReleaseTarget[]): string[] {
  const specs = targets.flatMap(target =>
    [...target.package.versionedFiles.map(f => f.path), target.changelog?.path].filter(isNotNil)
  );
  const shouldIncludeRootCargo = targets
    .flatMap(x => x.package.versionedFiles)
    .some(x => x.type === 'Cargo.toml');

  if (shouldIncludeRootCargo) {
    specs.push('Cargo.toml');
  }
  specs.push('CHANGELOG.md');
  return uniq(specs);
}

/**
 * A package's released identity for structured GitHub Actions output: its (next) version and the
 * registries it goes to. `release` extends this with the git tag and GitHub release; `prerelease`
 * uses it as-is. The version is read from `nextVersion`, which is the current version during
 * `release` (no pending bump) and the `-<id>.<sha>` prerelease version during `prerelease`.
 */
export interface ReleasedPackage {
  name: string;
  version: string;
  registries: VersionedFileRegistry[];
}

export function describeReleasedPackage(pkg: Package): ReleasedPackage {
  return {
    name: pkg.name,
    version: pkg.nextVersion.toString(),
    registries: pkg.versionedFiles.filter(file => file.canPublish).map(file => file.registry),
  };
}

export function logTarget(target: ReleaseTarget): void {
  const prefix = `[${target.package.name}]`;
  console.log(
    `${c.info(prefix)} ${target.package.version.toString()} -> ${c.success(
      target.package.nextVersion.toString()
    )}`
  );
  for (let i = 0; i < target.changes.changes.length; i += 1) {
    const change = target.changes.changes[i]!;
    const line = i === target.changes.changes.length - 1 ? '└─' : '├─';
    console.log(`   ${c.dim(`${line} ${change.toString()}`)}`);
  }
}
