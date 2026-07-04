import fs from 'node:fs/promises';
import { Command, Option } from 'clipanion';
import { openRepository, type Repository } from 'es-git';
import { type Action, runActions } from '../action.ts';
import { Changelog } from '../changelog.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';
import { resolveAssets } from '../github.ts';
import { Package } from '../package.ts';
import { createPorts, type GitHubRelease, type Ports } from '../ports.ts';
import {
  describeReleasedPackage,
  formatPublishStatus,
  type PublishOutcome,
  packagesBumpedInHead,
  publishPackage,
  type ReleasedPackage,
} from '../releasing.ts';

/** The GitHub release created for a package. */
interface ReleaseInfo {
  tag: string;
  url: string;
  assets: string[];
}

/** A released package (shared shape) plus the release-only git tag and GitHub release. */
interface PublishedPackage extends ReleasedPackage {
  tag: string;
  release: ReleaseInfo | null;
}

/**
 * Stable publish, run on the base branch after a Release PR merges.
 *
 * The targets are the packages whose version was raised by the merge's `HEAD` commit (the
 * `prepare-release` commit) — so an ordinary merge publishes nothing. Targets are published in
 * dependency order; each one that succeeds is tagged on the merge commit and gets a GitHub release
 * (with its configured assets). Every step skips work that is already done (registry versions,
 * git tags, GitHub releases, assets), so if some packages fail, the job exits non-zero and
 * re-running the same commit retries only what is still missing.
 */
export class ReleaseCommand extends Command {
  static paths = [['release']];

  readonly distTag = Option.String('--dist-tag', { required: false });
  readonly githubToken = Option.String('--github-token', { required: false, env: 'GITHUB_TOKEN' });
  readonly githubOutput = Option.String('--github-output', {
    required: false,
    env: 'GITHUB_OUTPUT',
  });
  readonly githubStepSummary = Option.String('--github-step-summary', {
    required: false,
    env: 'GITHUB_STEP_SUMMARY',
  });
  readonly check = Option.Boolean('--check', false);
  readonly dryRun = Option.Boolean('--dry-run', false);
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    const repo = await openRepository(ROOT_DIR);

    const targets = await this.findTargets(repo);
    // Whether `HEAD` is a release commit (a `prepare-release` PR merge that bumped versions). The
    // workflow uses this to choose `release` vs `prerelease`, independent of publish success.
    const isReleaseCommit = targets.length > 0;
    await this.setOutput('release_commit', isReleaseCommit ? 'true' : 'false');
    // The names of the packages this commit releases (always a valid JSON array, `[]` when none).
    // The workflow reads this to build per-package artifacts (e.g. ffi) only when needed.
    await this.setOutput('release_packages', JSON.stringify(targets.map(pkg => pkg.name)));

    if (this.check) {
      console.log(`${c.info('[root]')} release commit: ${isReleaseCommit}`);
      return 0;
    }

    if (targets.length === 0) {
      console.log(`${c.warn('[root]')} nothing to publish (no package was bumped by this commit).`);
      await this.setOutput('released', 'false');
      return 0;
    }

    const ports = createPorts({ repo, githubToken: this.githubToken });
    const head = repo.head().target();
    const outcomes = await this.publishPackages(targets, ports);
    const succeeded = outcomes.filter(x => x.status !== 'failed').map(x => x.package);
    // Per-package failures of the post-publish steps (tag/push/release), folded into the outcomes.
    const stepFailures = new Map<string, string>();
    let releases = new Map<string, ReleaseInfo>();
    if (succeeded.length > 0) {
      const tagged = await this.ensureTags(repo, succeeded, ports, stepFailures);
      if (tagged.length > 0) {
        if (await this.pushTags(tagged, ports)) {
          releases = await this.ensureGitHubReleases(
            tagged,
            head ?? undefined,
            ports,
            stepFailures
          );
        } else {
          // Stop before creating releases: with the tags missing from the remote, `createRelease`
          // would materialize them itself as lightweight tags.
          for (const pkg of tagged) {
            stepFailures.set(pkg.name, 'pushing tags failed');
          }
        }
      }
    }
    const finalOutcomes = outcomes.map((outcome): PublishOutcome => {
      const failure = stepFailures.get(outcome.package.name);
      return outcome.status !== 'failed' && failure != null
        ? { ...outcome, status: 'failed', reason: failure }
        : outcome;
    });

    await this.report(finalOutcomes, releases);
    return finalOutcomes.every(x => x.status !== 'failed') ? 0 : 1;
  }

  /**
   * The packages to release: those bumped by the `HEAD` (prepare-release) commit, in dependency
   * order so dependencies publish first. Already-released packages are *not* filtered out here —
   * each later step skips what is already done, so a re-run finishes whatever a partial failure
   * left behind. Packages that aren't published to a registry are included too — they are still
   * tagged and GitHub-released; only the registry push is skipped (per manifest).
   */
  private async findTargets(repo: Repository): Promise<Package[]> {
    const packages = await Package.loadAll();
    const graph = Package.buildGraph(packages);
    const targets = packagesBumpedInHead(repo, packages);
    return graph.topoSort(targets);
  }

  /** Publish each target package idempotently (see {@link publishPackage}); returns every outcome. */
  private async publishPackages(targets: Package[], ports: Ports): Promise<PublishOutcome[]> {
    const outcomes: PublishOutcome[] = [];
    for (const pkg of targets) {
      console.log(`${c.info(`[${pkg.name}]`)} publishing v${pkg.version.toString()}`);
      const outcome = await publishPackage(pkg, {
        current: true,
        distTag: this.distTag,
        dryRun: this.dryRun,
        ports,
      });
      if (outcome.status === 'failed') {
        console.error(`${c.error(`[${pkg.name}]`)} publish failed`);
      }
      outcomes.push(outcome);
    }
    return outcomes;
  }

  /** Create the missing git tags; returns the packages whose tag exists afterwards. */
  private async ensureTags(
    repo: Repository,
    packages: Package[],
    ports: Ports,
    failures: Map<string, string>
  ): Promise<Package[]> {
    const missing: Package[] = [];
    for (const pkg of packages) {
      if (pkg.versionedGitTag.exists(repo)) {
        console.log(`${c.warn('[root]')} tag already exists: ${pkg.versionedGitTag.tagName}`);
      } else {
        missing.push(pkg);
      }
    }
    const actions = missing.map(
      (pkg): Action => ({ type: 'createTag', tag: pkg.versionedGitTag.tagName })
    );
    const result = await runActions(actions, {
      dryRun: this.dryRun,
      ports,
      failFast: false,
      reject: false,
    });
    const failedTags = new Set(
      result.items
        .filter(item => !item.succeed)
        .map(item => (item.action.type === 'createTag' ? item.action.tag : ''))
    );
    for (const pkg of missing) {
      if (failedTags.has(pkg.versionedGitTag.tagName)) {
        failures.set(pkg.name, 'creating tag failed');
      }
    }
    return packages.filter(pkg => !failures.has(pkg.name));
  }

  /** Push the packages' tags; a failure is reported (not thrown) so the run can still report. */
  private async pushTags(packages: Package[], ports: Ports): Promise<boolean> {
    const refspecs = packages.map(pkg => {
      const ref = pkg.versionedGitTag.tagRef;
      return `${ref}:${ref}`;
    });
    const result = await runActions([{ type: 'pushTags', refspecs }], {
      dryRun: this.dryRun,
      ports,
      reject: false,
    });
    return result.allSucceed;
  }

  /**
   * Ensure a GitHub release exists per package (reusing one left by a previous attempt) and upload
   * its missing assets. Failures are collected per package instead of thrown, so one bad release
   * doesn't block the others (nor the report).
   */
  private async ensureGitHubReleases(
    packages: Package[],
    commitish: string | undefined,
    ports: Ports,
    failures: Map<string, string>
  ): Promise<Map<string, ReleaseInfo>> {
    const releases = new Map<string, ReleaseInfo>();
    for (const pkg of packages) {
      const tagName = pkg.versionedGitTag.tagName;
      // A throw while resolving assets/changelog is isolated to this package (like a failed
      // action), so one bad package can't abort the run before its report.
      try {
        const changelog = await Changelog.load(pkg.changelog).catch(() => null);
        const actions: Action[] = [
          {
            type: 'ensureRelease',
            tag: tagName,
            name: `${pkg.name} v${pkg.version.toString()}`,
            body: changelog?.extractChanges(pkg) ?? undefined,
            targetCommitish: commitish,
          },
          { type: 'uploadAssets', tag: tagName, assets: await resolveAssets(pkg) },
        ];
        const result = await runActions(actions, {
          name: pkg.name,
          dryRun: this.dryRun,
          ports,
          failFast: true,
          reject: false,
        });
        if (!result.allSucceed) {
          failures.set(pkg.name, 'github release failed');
          continue;
        }
        const release = result.items.find(item => item.action.type === 'ensureRelease');
        const uploaded = result.items.find(item => item.action.type === 'uploadAssets');
        const data = release?.succeed === true ? (release.data as GitHubRelease | undefined) : null;
        if (data != null) {
          releases.set(pkg.name, {
            tag: tagName,
            url: data.htmlUrl,
            assets:
              uploaded?.succeed === true ? ((uploaded.data as string[] | undefined) ?? []) : [],
          });
        }
      } catch (e) {
        console.error(`${c.error(`[${pkg.name}]`)} github release failed: ${(e as Error).message}`);
        failures.set(pkg.name, 'github release failed');
      }
    }
    return releases;
  }

  /**
   * Emit the `released` flag and a detailed `published` JSON array to the GitHub Actions output,
   * plus the step summary. The JSON describes each published package: its version, git tag, the
   * registries it was published to (npm/crates, with URLs), and its GitHub release + assets.
   */
  private async report(
    outcomes: PublishOutcome[],
    releases: Map<string, ReleaseInfo>
  ): Promise<void> {
    const published = outcomes.filter(x => x.status !== 'failed').map(x => x.package);
    const details = published.map(pkg => this.describe(pkg, releases.get(pkg.name) ?? null));
    await this.setOutput('released', published.length > 0 ? 'true' : 'false');
    await this.setOutput('published', JSON.stringify(details));
    await this.writeSummary(outcomes);
  }

  private describe(pkg: Package, release: ReleaseInfo | null): PublishedPackage {
    return { ...describeReleasedPackage(pkg), tag: pkg.versionedGitTag.tagName, release };
  }

  private async setOutput(key: string, value: string) {
    if (this.githubOutput == null) {
      return;
    }
    await fs.appendFile(this.githubOutput, `${key}=${value}\n`, 'utf8');
  }

  private async writeSummary(outcomes: PublishOutcome[]) {
    if (this.githubStepSummary == null || outcomes.length === 0) {
      return;
    }
    const rows = outcomes.map(outcome => {
      const pkg = outcome.package;
      return `| \`${pkg.name}\` | ${pkg.version.toString()} | ${formatPublishStatus(outcome)} |`;
    });
    const summary = [
      '## Release',
      '',
      '| package | version | status |',
      '| --- | --- | --- |',
      ...rows,
      '',
    ].join('\n');
    await fs.appendFile(this.githubStepSummary, summary, 'utf8');
  }
}
