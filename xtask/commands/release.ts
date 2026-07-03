import fs from 'node:fs/promises';
import { Command, Option } from 'clipanion';
import { openRepository, type Repository } from 'es-git';
import { Changelog } from '../changelog.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GIT_SIGNATURE, GITHUB_REPO, ROOT_DIR } from '../consts.ts';
import {
  createGitHubClient,
  findReleaseByTag,
  resolveAssets,
  uploadReleaseAssets,
} from '../github.ts';
import { Package } from '../package.ts';
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
 * The targets are the packages whose version was changed by the merge's `HEAD` commit (the
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

    const head = repo.head().target();
    const outcomes = await this.publishPackages(targets);
    const succeeded = outcomes.filter(x => x.status !== 'failed').map(x => x.package);
    let releases: Map<string, ReleaseInfo> = new Map();
    let releaseFailures: Map<string, string> = new Map();
    if (succeeded.length > 0) {
      this.createTags(repo, succeeded);
      if (await this.pushTags(repo, succeeded)) {
        ({ releases, failures: releaseFailures } = await this.createGitHubReleases(
          succeeded,
          head ?? undefined
        ));
      } else {
        // Stop before creating releases: with the tags missing from the remote, `createRelease`
        // would materialize them itself as lightweight tags.
        for (const pkg of succeeded) {
          releaseFailures.set(pkg.name, 'pushing tags failed');
        }
      }
    }
    // Fold GitHub release failures into the outcomes so they fail the run (and get retried).
    const finalOutcomes = outcomes.map((outcome): PublishOutcome => {
      const failure = releaseFailures.get(outcome.package.name);
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
  private async publishPackages(targets: Package[]): Promise<PublishOutcome[]> {
    const outcomes: PublishOutcome[] = [];
    for (const pkg of targets) {
      console.log(`${c.info(`[${pkg.name}]`)} publishing v${pkg.version.toString()}`);
      const outcome = await publishPackage(pkg, {
        current: true,
        distTag: this.distTag,
        dryRun: this.dryRun,
      });
      if (outcome.status === 'failed') {
        console.error(`${c.error(`[${pkg.name}]`)} publish failed`);
      }
      outcomes.push(outcome);
    }
    return outcomes;
  }

  private createTags(repo: Repository, packages: Package[]) {
    const head = repo.head().target();
    if (head == null || this.dryRun) {
      for (const pkg of packages) {
        console.log(`${c.info(`[${pkg.name}]`)} will create tag: ${pkg.versionedGitTag.tagName}`);
      }
      return;
    }
    const commit = repo.getCommit(head);
    for (const pkg of packages) {
      const tag = pkg.versionedGitTag;
      if (tag.exists(repo)) {
        console.log(`${c.warn('[root]')} tag already exists: ${tag.tagName}`);
        continue;
      }
      const tagId = repo.createTag(tag.tagName, commit.asObject(), tag.tagName, {
        tagger: GIT_SIGNATURE,
      });
      console.log(`${c.success('[root]')} tag: ${repo.getTag(tagId).name()}`);
    }
  }

  /** Push the packages' tags; a failure is reported (not thrown) so the run can still report. */
  private async pushTags(repo: Repository, packages: Package[]): Promise<boolean> {
    const refspecs = packages.map(pkg => {
      const ref = pkg.versionedGitTag.tagRef;
      return `${ref}:${ref}`;
    });
    if (this.dryRun || this.githubToken == null) {
      console.log(`${c.info('[root]')} will push tags:`);
      for (const ref of refspecs) {
        console.log(c.dim(`  - ${ref}`));
      }
      return true;
    }
    try {
      const remote = repo.getRemote('origin');
      await remote.push(refspecs, { credential: { type: 'Plain', password: this.githubToken } });
      console.log(`${c.success('[root]')} pushed ${refspecs.length} tag(s)`);
      return true;
    } catch (e) {
      console.error(`${c.error('[root]')} failed to push tags: ${(e as Error).message}`);
      return false;
    }
  }

  /**
   * Ensure a GitHub release exists per package (reusing one left by a previous attempt) and upload
   * its missing assets. Failures are collected per package instead of thrown, so one bad release
   * doesn't block the others (nor the report).
   */
  private async createGitHubReleases(
    packages: Package[],
    commitish: string | undefined
  ): Promise<{ releases: Map<string, ReleaseInfo>; failures: Map<string, string> }> {
    const releases = new Map<string, ReleaseInfo>();
    const failures = new Map<string, string>();
    const client = this.githubToken != null ? createGitHubClient(this.githubToken) : null;
    for (const pkg of packages) {
      const tagName = pkg.versionedGitTag.tagName;
      if (this.dryRun || client == null) {
        console.log(`${c.info('[root]')} will create github release: ${tagName}`);
        for (const asset of pkg.assets) {
          console.log(c.dim(`  will upload asset: ${asset}`));
        }
        continue;
      }
      try {
        let release = await findReleaseByTag(client, tagName);
        if (release != null) {
          console.log(`${c.warn('[root]')} github release already exists: ${tagName}`);
        } else {
          const changelog = await Changelog.load(pkg.changelog).catch(() => null);
          const created = await client.rest.repos.createRelease({
            owner: GITHUB_REPO.owner,
            repo: GITHUB_REPO.name,
            tag_name: tagName,
            // Pins a tag GitHub might still need to create to the release commit.
            target_commitish: commitish,
            name: `${pkg.name} v${pkg.version.toString()}`,
            body: changelog?.extractChanges(pkg) ?? undefined,
          });
          console.log(`${c.success('[root]')} github release: ${created.data.tag_name}`);
          release = { id: created.data.id, htmlUrl: created.data.html_url };
        }
        const assets = await uploadReleaseAssets(client, release.id, await resolveAssets(pkg));
        releases.set(pkg.name, { tag: tagName, url: release.htmlUrl, assets });
      } catch (e) {
        console.error(`${c.error(`[${pkg.name}]`)} github release failed: ${(e as Error).message}`);
        failures.set(pkg.name, 'github release failed');
      }
    }
    return { releases, failures };
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
