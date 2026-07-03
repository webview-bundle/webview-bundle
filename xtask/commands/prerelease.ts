import fs from 'node:fs/promises';
import { Command, Option } from 'clipanion';
import { type Commit, openRepository } from 'es-git';
import { Changelog } from '../changelog.ts';
import { Change, Changes } from '../changes.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GITHUB_REPO, ROOT_DIR } from '../consts.ts';
import {
  createGitHubClient,
  findReleaseByTag,
  resolveAssets,
  uploadReleaseAssets,
} from '../github.ts';
import type { Package, PackageGraph } from '../package.ts';
import {
  describeReleasedPackage,
  formatPublishStatus,
  logTarget,
  type PublishOutcome,
  planRelease,
  publishPackage,
  type ReleasePlan,
  type ReleaseTarget,
  writeReleaseTargets,
} from '../releasing.ts';

/**
 * Verification prerelease, run on every base-branch commit.
 *
 * Finds packages changed since their last tag (directly, or via the dependency graph), bumps them
 * to `x.y.z-<id>.<short-sha>`, writes the version + changelog (without committing — `cargo publish`
 * runs with `--allow-dirty`), and publishes the registry-publishable ones under the `<id>` channel
 * (npm dist-tag). Every affected package's assets — including the Android/Swift FFI artifacts that
 * aren't published to a registry — are uploaded to a single `prerelease: true` GitHub release
 * tagged `prerelease/<sha>`, so prerelease builds can be downloaded from a separate repo. Every
 * target's publish status (including failures) is reported via GitHub Actions output and the
 * step summary.
 *
 * The prerelease version is derived from the commit (`<id>.<short-sha>`), so re-running the job
 * on the same commit retries only what is missing: already-published versions are skipped, the
 * existing GitHub release is reused, and only its missing assets are uploaded.
 */
export class PrereleaseCommand extends Command {
  static paths = [['prerelease']];

  readonly id = Option.String('--id', 'next');
  readonly githubToken = Option.String('--github-token', { required: false, env: 'GITHUB_TOKEN' });
  readonly githubOutput = Option.String('--github-output', {
    required: false,
    env: 'GITHUB_OUTPUT',
  });
  readonly githubStepSummary = Option.String('--github-step-summary', {
    required: false,
    env: 'GITHUB_STEP_SUMMARY',
  });
  readonly dryRun = Option.Boolean('--dry-run', false);
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    const repo = await openRepository(ROOT_DIR);

    const plan = await planRelease(repo);
    if (plan.candidates.length === 0) {
      console.log(`${c.warn('[root]')} no changes since last release. nothing to prerelease.`);
      await this.setOutput('prereleased', 'false');
      return 0;
    }

    const head = repo.head().target();
    if (head == null) {
      throw new Error('cannot resolve git `HEAD`');
    }
    const sha = head.slice(0, 7);
    const tag = `prerelease/${sha}`;

    // 1) Registry prereleases: bump + publish only packages that publish to a registry.
    const publishable = plan.candidates.filter(pkg => pkg.canPublish);
    const targets = await this.buildTargets(plan, publishable, sha);
    for (const target of targets) {
      logTarget(target);
    }
    // Write bumped versions + changelogs so the publish carries them. Not committed.
    await writeReleaseTargets(targets, { dryRun: this.dryRun });
    const outcomes = await this.publishTargets(targets);
    const published = outcomes.filter(x => x.status !== 'failed').map(x => x.package);

    // 2) One aggregated GitHub prerelease hosting every affected package's assets. A failure here
    // must not prevent the report below.
    let assetsFailed = false;
    let assetReleaseUrl: string | null = null;
    try {
      assetReleaseUrl = await this.uploadPrereleaseAssets(
        plan.candidates,
        head,
        sha,
        tag,
        published
      );
    } catch (e) {
      assetsFailed = true;
      console.error(
        `${c.error('[root]')} failed to upload prerelease assets: ${(e as Error).message}`
      );
    }

    await this.report(outcomes, assetReleaseUrl);
    const allPublished = outcomes.every(x => x.status !== 'failed');
    return allPublished && !assetsFailed ? 0 : 1;
  }

  /** Bump every (publishable) candidate to a prerelease of its current version and build its changelog. */
  private async buildTargets(
    plan: ReleasePlan,
    candidates: Package[],
    build: string
  ): Promise<ReleaseTarget[]> {
    const affected = new Set(candidates);
    const targets: ReleaseTarget[] = [];
    for (const pkg of candidates) {
      pkg.bumpPrerelease(this.id, build);
      const changes = this.buildChanges(
        plan.graph,
        pkg,
        plan.directChanges.get(pkg) ?? [],
        affected
      );
      const changelog = await Changelog.load(pkg.changelog).catch(() => null);
      targets.push({ package: pkg, changes, changelog });
    }
    return targets;
  }

  /** A package's changelog entries: all of its own commits plus an automatic dependency-bump note. */
  private buildChanges(
    graph: PackageGraph,
    pkg: Package,
    commits: Commit[],
    affected: Set<Package>
  ): Changes {
    const changes = commits.map(commit => new Change(commit.summary() ?? '', commit.id()));
    const changedDeps = graph.dependenciesOf(pkg).filter(dep => affected.has(dep));
    if (changedDeps.length > 0) {
      const note = changedDeps
        .flatMap(dep => dep.versionFileNames.map(name => `${name}@${dep.nextVersion.toString()}`))
        .join(', ');
      changes.push(new Change(`update dependencies: ${note}`));
    }
    return new Changes(changes);
  }

  /** Publish each target idempotently (see {@link publishPackage}); returns every outcome. */
  private async publishTargets(targets: ReleaseTarget[]): Promise<PublishOutcome[]> {
    const outcomes: PublishOutcome[] = [];
    for (const { package: pkg } of targets) {
      const outcome = await publishPackage(pkg, { dryRun: this.dryRun });
      if (outcome.status === 'failed') {
        console.error(`${c.error(`[${pkg.name}]`)} prerelease publish failed`);
      }
      outcomes.push(outcome);
    }
    return outcomes;
  }

  /**
   * Collect every affected package's assets and upload them to one GitHub release tagged
   * `prerelease/<sha>` (marked `prerelease: true`), so prerelease builds — notably the Android/Swift
   * FFI artifacts that aren't published to a registry — can be downloaded. A release left by a
   * previous run is reused: its body is refreshed and only the missing assets are uploaded.
   * Returns the release URL, or `null` when there is nothing to upload (or no token / dry-run).
   */
  private async uploadPrereleaseAssets(
    candidates: Package[],
    commitish: string,
    sha: string,
    tag: string,
    published: Package[]
  ): Promise<string | null> {
    const assets = (await Promise.all(candidates.map(pkg => resolveAssets(pkg)))).flat();
    if (assets.length === 0) {
      console.log(`${c.warn('[root]')} no assets found. skip prerelease release.`);
      return null;
    }
    if (this.dryRun || this.githubToken == null) {
      console.log(
        `${c.info('[root]')} will create prerelease "${tag}" with ${assets.length} asset(s):`
      );
      for (const asset of assets) {
        console.log(c.dim(`  ${asset.name}`));
      }
      return null;
    }

    const client = createGitHubClient(this.githubToken);
    const repo = { owner: GITHUB_REPO.owner, repo: GITHUB_REPO.name };
    const existing = await findReleaseByTag(client, tag);
    let releaseId: number;
    let releaseUrl: string;
    if (existing != null) {
      console.log(`${c.warn('[root]')} prerelease release "${tag}" already exists. reusing it.`);
      await client.rest.repos.updateRelease({
        ...repo,
        release_id: existing.id,
        body: this.prereleaseBody(sha, published),
      });
      releaseId = existing.id;
      releaseUrl = existing.htmlUrl;
    } else {
      const release = await client.rest.repos.createRelease({
        ...repo,
        tag_name: tag,
        target_commitish: commitish,
        name: `prerelease ${sha}`,
        body: this.prereleaseBody(sha, published),
        prerelease: true,
      });
      console.log(`${c.success('[root]')} prerelease release: ${release.data.tag_name}`);
      releaseId = release.data.id;
      releaseUrl = release.data.html_url;
    }
    await uploadReleaseAssets(client, releaseId, assets);
    return releaseUrl;
  }

  private prereleaseBody(sha: string, published: Package[]): string {
    const lines = [`Prerelease build for commit \`${sha}\`.`, ''];
    const registries = published.flatMap(pkg => describeReleasedPackage(pkg).registries);
    if (registries.length > 0) {
      lines.push('Published to registries:', '');
      for (const registry of registries) {
        lines.push(`- \`${registry.name}@${registry.version}\` (${registry.type})`);
      }
    }
    return lines.join('\n');
  }

  /**
   * Report every target's publish status (including failures) via the GitHub Actions output and
   * step summary. The `packages` output keeps carrying only the published packages.
   */
  private async report(outcomes: PublishOutcome[], assetReleaseUrl: string | null): Promise<void> {
    const published = outcomes.filter(x => x.status !== 'failed');
    const entries = published.map(x => describeReleasedPackage(x.package));
    await this.setOutput('prereleased', published.length > 0 ? 'true' : 'false');
    await this.setOutput('packages', JSON.stringify(entries));
    if (assetReleaseUrl != null) {
      await this.setOutput('assets', assetReleaseUrl);
    }
    if (this.githubStepSummary != null && (outcomes.length > 0 || assetReleaseUrl != null)) {
      const summary = [
        '## Prerelease',
        '',
        ...(outcomes.length > 0
          ? [
              '| package | version | status |',
              '| --- | --- | --- |',
              ...outcomes.map(
                outcome =>
                  `| \`${outcome.package.name}\` | \`${outcome.package.nextVersion.toString()}\` | ${formatPublishStatus(outcome)} |`
              ),
              '',
            ]
          : []),
        ...(assetReleaseUrl != null ? [`Assets: ${assetReleaseUrl}`, ''] : []),
      ].join('\n');
      await fs.appendFile(this.githubStepSummary, summary, 'utf8');
    }
  }

  private async setOutput(key: string, value: string): Promise<void> {
    if (this.githubOutput == null) {
      return;
    }
    await fs.appendFile(this.githubOutput, `${key}=${value}\n`, 'utf8');
  }
}
