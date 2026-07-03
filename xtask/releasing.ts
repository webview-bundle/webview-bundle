import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { Commit, Repository, Tree } from 'es-git';
import { isNotNil, uniq } from 'es-toolkit';
import type { PackageJson } from 'type-fest';
import { type Action, runActions } from './action.ts';
import { editCargoTomlVersion, formatCargoToml, parseCargoToml } from './cargo-toml.ts';
import { Changelog } from './changelog.ts';
import type { Changes } from './changes.ts';
import { c } from './console.ts';
import { ROOT_DIR } from './consts.ts';
import { commitsTouchingPaths } from './git.ts';
import { Package, type PackageGraph } from './package.ts';
import { defaultPorts, type Ports } from './ports.ts';
import { Version } from './version.ts';
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
 * Packages whose version was *raised* by the `HEAD` commit, compared to its first parent — i.e.
 * the packages bumped by a merged `prepare-release` commit. A package counts only if one of its
 * manifests existed in the parent with a *lower* version: a manifest that is absent from the
 * parent is a brand-new package (or a newly added manifest), not a bump, so the commit that first
 * introduces a package does not publish it. Such a package is released through the normal
 * `prepare-release` flow, whose merge commit bumps the (now-existing) manifest off its initial
 * version. Requiring the version to go *up* (not merely change) keeps a revert of a release merge
 * from re-triggering the release pipeline. This is how `release` decides what to publish, so an
 * ordinary merge (no version change) publishes nothing.
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
      if (previous == null) {
        return false;
      }
      try {
        return file.version.greaterThan(Version.parse(previous));
      } catch {
        return false;
      }
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

export type PublishStatus = 'published' | 'already-published' | 'failed';

/** The result of one package's (idempotent) publish attempt. */
export interface PublishOutcome {
  package: Package;
  status: PublishStatus;
  /** What failed, when `status === 'failed'` (the beforePublish scripts vs the publish itself). */
  reason?: string;
}

export interface PublishPackageOptions {
  /** Publish the manifests' current (already-written) version instead of the pending bump. */
  current?: boolean;
  distTag?: string;
  dryRun?: boolean;
  ports?: Ports;
}

/** A publishable manifest's target version and whether it is already live in its registry. */
export interface ManifestPublishState {
  file: VersionedFile;
  version: Version;
  exists: boolean | null;
}

/**
 * Observe which of `pkg`'s publishable manifest versions are already live in their registry.
 * Dry runs stay offline (every version is reported missing, so the plan shows every publish).
 */
export async function observePublishState(
  pkg: Package,
  opts: PublishPackageOptions = {}
): Promise<ManifestPublishState[]> {
  const { current = false, dryRun = false, ports = defaultPorts } = opts;
  const files = pkg.versionedFiles.filter(file => file.canPublish && (current || file.hasChanged));
  const states: ManifestPublishState[] = [];
  for (const file of files) {
    const version = current ? file.version : file.nextVersion;
    const exists = dryRun
      ? false
      : await ports.registry.exists(file.type, file.name, version.toString());
    if (exists === true) {
      console.log(
        `${c.warn(`[${pkg.name}]`)} ${file.name}@${version.toString()} already published. skip.`
      );
    }
    states.push({ file, version, exists });
  }
  return states;
}

/** What publishing one package still requires. */
export interface PackagePublishPlan {
  pkg: Package;
  /** The `beforePublish` scripts; empty when nothing needs publishing. */
  scripts: Action[];
  /** The publishes still missing from their registry. */
  publishes: Action[];
  /** Publishable manifests exist, but every version is already live (pure retry no-op). */
  alreadyPublished: boolean;
}

/**
 * Decide what publishing `pkg` still requires, from the observed registry state. Pure. Skipping
 * already-live versions is what makes `release`/`prerelease` retryable: re-running after a
 * partial failure publishes only what is still missing instead of failing on "version already
 * exists". The `beforePublish` scripts are planned only when at least one publish remains.
 */
export function planPackagePublish(
  pkg: Package,
  manifests: ManifestPublishState[],
  opts: { distTag?: string } = {}
): PackagePublishPlan {
  const pending = manifests.filter(manifest => manifest.exists !== true);
  const publishes = pending.map(manifest =>
    manifest.file.publishAction(manifest.version, opts.distTag)
  );
  const scripts: Action[] =
    pending.length === 0
      ? []
      : pkg.beforePublishScripts.map(script => ({
          type: 'command',
          cmd: script.command,
          args: (script.args ?? []) as string[],
          path: script.cwd ?? pkg.path,
        }));
  return {
    pkg,
    scripts,
    publishes,
    alreadyPublished: manifests.length > 0 && pending.length === 0,
  };
}

/** Execute a package's publish plan; duplicate-version rejections count as already published. */
export async function applyPackagePublish(
  plan: PackagePublishPlan,
  opts: { dryRun?: boolean; ports?: Ports } = {}
): Promise<PublishOutcome> {
  const { pkg, scripts, publishes, alreadyPublished } = plan;
  const { dryRun = false, ports = defaultPorts } = opts;
  if (alreadyPublished) {
    return { package: pkg, status: 'already-published' };
  }
  if (publishes.length === 0) {
    // Nothing goes to a registry (e.g. a private package that is only tagged + GitHub-released).
    return { package: pkg, status: 'published' };
  }
  if (scripts.length > 0) {
    const result = await runActions(scripts, { name: pkg.name, dryRun, ports, reject: false });
    if (!result.allSucceed) {
      console.error(`${c.error(`[${pkg.name}]`)} beforePublish scripts failed`);
      return { package: pkg, status: 'failed', reason: 'beforePublish scripts failed' };
    }
  }
  const result = await runActions(publishes, {
    name: pkg.name,
    dryRun,
    ports,
    failFast: false,
    reject: false,
  });
  if (!result.allSucceed) {
    return { package: pkg, status: 'failed', reason: 'publish failed' };
  }
  return { package: pkg, status: 'published' };
}

/** Observe → plan → apply one package's publish. */
export async function publishPackage(
  pkg: Package,
  opts: PublishPackageOptions = {}
): Promise<PublishOutcome> {
  const manifests = await observePublishState(pkg, opts);
  const plan = planPackagePublish(pkg, manifests, opts);
  return applyPackagePublish(plan, opts);
}

/** The status cell shown for a package in the GitHub step summary. */
export function formatPublishStatus(outcome: PublishOutcome): string {
  switch (outcome.status) {
    case 'published':
      return '✅ published';
    case 'already-published':
      return '✅ already published';
    case 'failed':
      return `❌ ${outcome.reason ?? 'failed'}`;
  }
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
