import fs from 'node:fs/promises';
import { Command, Option } from 'clipanion';
import { openRepository, type Repository } from 'es-git';
import { runActions } from '../action.ts';
import { Changelog } from '../changelog.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GIT_SIGNATURE, GITHUB_REPO, ROOT_DIR } from '../consts.ts';
import { createGitHubClient, resolveAssets, uploadReleaseAssets } from '../github.ts';
import { Package } from '../package.ts';
import {
  describeReleasedPackage,
  packagesBumpedInHead,
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
 * (with its configured assets). Tagging is what marks a package "released", so this is idempotent:
 * if some packages fail, the job exits non-zero and re-running the same commit retries only the
 * still-untagged ones.
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

    if (this.check) {
      console.log(`${c.info('[root]')} release commit: ${isReleaseCommit}`);
      return 0;
    }

    if (targets.length === 0) {
      console.log(`${c.warn('[root]')} nothing to publish (no package was bumped by this commit).`);
      await this.setOutput('released', 'false');
      return 0;
    }

    const published = await this.publishPackages(targets);
    let releases: Map<string, ReleaseInfo> = new Map();
    if (published.length > 0) {
      this.createTags(repo, published);
      await this.pushTags(repo, published);
      releases = await this.createGitHubReleases(published);
    }

    await this.report(published, targets, releases);
    return published.length === targets.length ? 0 : 1;
  }

  /**
   * The packages to release: those bumped by the `HEAD` (prepare-release) commit, minus any whose
   * version is already tagged (so a re-run retries only what failed). Returned in dependency order
   * so dependencies publish first. Packages that aren't published to a registry are included too —
   * they are still tagged and GitHub-released (e.g. for their assets); only the per-manifest
   * registry push is skipped during `publishCurrent`.
   */
  private async findTargets(repo: Repository): Promise<Package[]> {
    const packages = await Package.loadAll();
    const graph = Package.buildGraph(packages);
    const targets = packagesBumpedInHead(repo, packages).filter(
      pkg => !pkg.versionedGitTag.exists(repo)
    );
    return graph.topoSort(targets);
  }

  /** Publish each target package (running its beforePublish scripts first); returns the succeeded. */
  private async publishPackages(targets: Package[]): Promise<Package[]> {
    const published: Package[] = [];
    for (const pkg of targets) {
      console.log(`${c.info(`[${pkg.name}]`)} publishing v${pkg.version.toString()}`);
      if (!(await this.runBeforePublish(pkg))) {
        continue;
      }
      const result = await runActions(pkg.publishCurrent(this.distTag), {
        name: pkg.name,
        dryRun: this.dryRun,
        failFast: false,
        reject: false,
      });
      if (result.allSucceed) {
        published.push(pkg);
      } else {
        console.error(`${c.error(`[${pkg.name}]`)} publish failed`);
      }
    }
    return published;
  }

  private async runBeforePublish(pkg: Package): Promise<boolean> {
    if (pkg.beforePublishScripts.length === 0) {
      return true;
    }
    const result = await runActions(
      pkg.beforePublishScripts.map(script => ({
        type: 'command' as const,
        cmd: script.command,
        args: (script.args ?? []) as string[],
        path: script.cwd ?? pkg.path,
      })),
      { name: pkg.name, dryRun: this.dryRun, reject: false }
    );
    if (!result.allSucceed) {
      console.error(`${c.error(`[${pkg.name}]`)} beforePublish scripts failed`);
      return false;
    }
    return true;
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
      const tagId = repo.createTag(tag.tagName, commit.asObject(), tag.tagName, {
        tagger: GIT_SIGNATURE,
      });
      console.log(`${c.success('[root]')} tag: ${repo.getTag(tagId).name()}`);
    }
  }

  private async pushTags(repo: Repository, packages: Package[]) {
    const refspecs = packages.map(pkg => {
      const ref = pkg.versionedGitTag.tagRef;
      return `${ref}:${ref}`;
    });
    if (this.dryRun || this.githubToken == null) {
      console.log(`${c.info('[root]')} will push tags:`);
      for (const ref of refspecs) {
        console.log(c.dim(`  - ${ref}`));
      }
      return;
    }
    const remote = repo.getRemote('origin');
    await remote.push(refspecs, { credential: { type: 'Plain', password: this.githubToken } });
    console.log(`${c.success('[root]')} pushed ${refspecs.length} tag(s)`);
  }

  /** Create a GitHub release per package; returns the release URL + uploaded assets, keyed by name. */
  private async createGitHubReleases(packages: Package[]): Promise<Map<string, ReleaseInfo>> {
    const releases = new Map<string, ReleaseInfo>();
    const client = this.githubToken != null ? createGitHubClient(this.githubToken) : null;
    for (const pkg of packages) {
      const changelog = await Changelog.load(pkg.changelog).catch(() => null);
      const tagName = pkg.versionedGitTag.tagName;
      const payload = {
        tag_name: tagName,
        name: `${pkg.name} v${pkg.version.toString()}`,
        body: changelog?.extractChanges(pkg) ?? undefined,
      };
      if (this.dryRun || client == null) {
        console.log(`${c.info('[root]')} will create github release: ${tagName}`);
        for (const asset of pkg.assets) {
          console.log(c.dim(`  will upload asset: ${asset}`));
        }
        continue;
      }
      const release = await client.rest.repos.createRelease({
        owner: GITHUB_REPO.owner,
        repo: GITHUB_REPO.name,
        ...payload,
      });
      console.log(`${c.success('[root]')} github release: ${release.data.tag_name}`);
      const assets = await uploadReleaseAssets(client, release.data.id, await resolveAssets(pkg));
      releases.set(pkg.name, { tag: tagName, url: release.data.html_url, assets });
    }
    return releases;
  }

  /**
   * Emit the `released` flag and a detailed `published` JSON array to the GitHub Actions output,
   * plus the step summary. The JSON describes each published package: its version, git tag, the
   * registries it was published to (npm/crates, with URLs), and its GitHub release + assets.
   */
  private async report(
    published: Package[],
    targets: Package[],
    releases: Map<string, ReleaseInfo>
  ): Promise<void> {
    const details = published.map(pkg => this.describe(pkg, releases.get(pkg.name) ?? null));
    await this.setOutput('released', published.length > 0 ? 'true' : 'false');
    await this.setOutput('published', JSON.stringify(details));
    await this.writeSummary(published, targets);
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

  private async writeSummary(published: Package[], targets: Package[]) {
    if (this.githubStepSummary == null || targets.length === 0) {
      return;
    }
    const publishedNames = new Set(published.map(pkg => pkg.name));
    const rows = targets.map(pkg => {
      const status = publishedNames.has(pkg.name) ? '✅ published' : '❌ failed';
      return `| \`${pkg.name}\` | ${pkg.version.toString()} | ${status} |`;
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
