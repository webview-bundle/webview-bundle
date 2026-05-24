import fs from 'node:fs/promises';
import { Command, Option } from 'clipanion';
import { type Commit, openRepository } from 'es-git';
import { type Action, runActions } from '../action.ts';
import { Changelog } from '../changelog.ts';
import { Change, Changes } from '../changes.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GITHUB_REPO, ROOT_DIR } from '../consts.ts';
import { createGitHubClient, resolveAssets, uploadReleaseAssets } from '../github.ts';
import type { Package, PackageGraph } from '../package.ts';
import {
  describeReleasedPackage,
  logTarget,
  planRelease,
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
 * tagged `prerelease/<sha>`, so prerelease builds can be downloaded from a separate repo. The set
 * of prereleased packages is reported via GitHub Actions output and the step summary.
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

    if (await this.alreadyReleased(tag)) {
      console.log(`${c.warn('[root]')} "${tag}" already exists. nothing to prerelease.`);
      await this.setOutput('prereleased', 'false');
      return 0;
    }

    // 1) Registry prereleases: bump + publish only packages that publish to a registry.
    const publishable = plan.candidates.filter(pkg => pkg.canPublish);
    const targets = await this.buildTargets(plan, publishable, sha);
    for (const target of targets) {
      logTarget(target);
    }
    // Write bumped versions + changelogs so the publish carries them. Not committed.
    await writeReleaseTargets(targets, { dryRun: this.dryRun });
    const published = await this.publishTargets(targets);

    // 2) One aggregated GitHub prerelease hosting every affected package's assets.
    const assetReleaseUrl = await this.uploadPrereleaseAssets(
      plan.candidates,
      head,
      sha,
      tag,
      published
    );

    await this.report(published, assetReleaseUrl);
    return published.length === targets.length ? 0 : 1;
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

  /** Publish each target (running its beforePublish scripts first); returns the ones that succeeded. */
  private async publishTargets(targets: ReleaseTarget[]): Promise<Package[]> {
    const published: Package[] = [];
    for (const { package: pkg } of targets) {
      if (!(await this.runBeforePublish(pkg))) {
        continue;
      }
      const result = await runActions(pkg.publish(), {
        name: pkg.name,
        dryRun: this.dryRun,
        failFast: false,
        reject: false,
      });
      if (result.allSucceed) {
        published.push(pkg);
      } else {
        console.error(`${c.error(`[${pkg.name}]`)} prerelease publish failed`);
      }
    }
    return published;
  }

  private async runBeforePublish(pkg: Package): Promise<boolean> {
    if (pkg.beforePublishScripts.length === 0) {
      return true;
    }
    const result = await runActions(
      pkg.beforePublishScripts.map(
        (script): Action => ({
          type: 'command',
          cmd: script.command,
          args: (script.args ?? []) as string[],
          path: script.cwd ?? pkg.path,
        })
      ),
      { name: pkg.name, dryRun: this.dryRun, reject: false }
    );
    if (!result.allSucceed) {
      console.error(`${c.error(`[${pkg.name}]`)} beforePublish scripts failed`);
      return false;
    }
    return true;
  }

  /**
   * Collect every affected package's assets and upload them to one GitHub release tagged
   * `prerelease/<sha>` (marked `prerelease: true`), so prerelease builds — notably the Android/Swift
   * FFI artifacts that aren't published to a registry — can be downloaded. Returns the release URL,
   * or `null` when there is nothing to upload (or no token / dry-run).
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
    const release = await client.rest.repos.createRelease({
      ...repo,
      tag_name: tag,
      target_commitish: commitish,
      name: `prerelease ${sha}`,
      body: this.prereleaseBody(sha, published),
      prerelease: true,
    });
    console.log(`${c.success('[root]')} prerelease release: ${release.data.tag_name}`);
    await uploadReleaseAssets(client, release.data.id, assets);
    return release.data.html_url;
  }

  private async alreadyReleased(tag: string): Promise<boolean> {
    if (this.dryRun || this.githubToken == null) {
      return false;
    }
    const client = createGitHubClient(this.githubToken);
    const repo = { owner: GITHUB_REPO.owner, repo: GITHUB_REPO.name };
    // getRef answers 404 when the ref does not exist.
    const ref = await client.rest.git.getRef({ ...repo, ref: `tags/${tag}` }).catch(() => null);
    return ref != null;
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

  /** Report the prereleased packages + the assets release via the GitHub Actions output and summary. */
  private async report(published: Package[], assetReleaseUrl: string | null): Promise<void> {
    const entries = published.map(pkg => describeReleasedPackage(pkg));
    await this.setOutput('prereleased', published.length > 0 ? 'true' : 'false');
    await this.setOutput('packages', JSON.stringify(entries));
    if (assetReleaseUrl != null) {
      await this.setOutput('assets', assetReleaseUrl);
    }
    if (this.githubStepSummary != null && (entries.length > 0 || assetReleaseUrl != null)) {
      const summary = [
        '## Prerelease',
        '',
        ...(entries.length > 0
          ? [
              '| package | version |',
              '| --- | --- |',
              ...entries.map(entry => `| \`${entry.name}\` | \`${entry.version}\` |`),
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
