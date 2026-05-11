import fs from 'node:fs/promises';
import path from 'node:path';
import { checkbox } from '@inquirer/prompts';
import { retry } from '@octokit/plugin-retry';
import { Octokit } from '@octokit/rest';
import { isCI } from 'ci-info';
import { Command, Option } from 'clipanion';
import { openRepository, type Repository } from 'es-git';
import { differenceBy, isNotNil, omit } from 'es-toolkit';
import { type Action, runActions } from '../action.ts';
import { editCargoTomlVersion, formatCargoToml, parseCargoToml } from '../cargo-toml.ts';
import { Changelog } from '../changelog.ts';
import { Changes } from '../changes.ts';
import { type Config, loadConfig } from '../config.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';
import { GIT_SIGNATURE } from '../git.ts';
import { Package } from '../package.ts';
import { loadStaged, removeStaged, type Staged, saveStaged } from '../staged.ts';
import { parsePrerelease } from '../version.ts';

interface ReleaseTarget {
  package: Package;
  changes: Changes;
  changelog: Changelog | null;
}

export class Release extends Command {
  static paths = [['release']];

  readonly configFilepath = Option.String('--config', 'xtask.json');
  readonly stagedFilepath = Option.String('--staged', 'xtask/.gen/staged.json');
  readonly dryRun = Option.Boolean('--dry-run', false);
  readonly githubToken = Option.String('--github-token', {
    required: false,
    env: 'GITHUB_TOKEN',
  });
  readonly prerelease = Option.String('--prerelease');
  readonly interactive = Option.Boolean('--interactive', !isCI);
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    const config = await loadConfig(this.configFilepath);
    const staged = await loadStaged(this.stagedFilepath);
    const repo = await openRepository(ROOT_DIR);
    let targets = await this.prepareTargets(repo, config, staged);
    if (targets.length === 0) {
      return 0;
    }
    if (this.interactive) {
      targets = await this.selectTargets(targets);
    }

    await this.writeReleaseTarget(targets);
    const rootCargoChanged = await this.writeRootCargoToml(targets);
    const rootChangelogChanged = await this.writeRootChangelog(config, targets);

    const publishedTargets = await this.publish(targets);
    const isAllPublished = publishedTargets.length === targets.length;

    // If-all failed, exit with code 1
    if (publishedTargets.length === 0) {
      return 1;
    }

    if (this.prerelease == null) {
      if (!this.dryRun) {
        await this.updateStaged(publishedTargets, staged);
      }
      this.gitEnsureMainBranch(repo);
      const commitId = this.gitCommitChanges(
        repo,
        config,
        publishedTargets,
        rootCargoChanged,
        rootChangelogChanged
      );
      this.gitCreateTags(repo, commitId, publishedTargets);
      await this.gitPush(repo, publishedTargets);
      await this.createGitHubReleases(config, publishedTargets);
    }

    if (!isAllPublished) {
      const failedTargets = differenceBy(targets, publishedTargets, x => x.package.name);
      for (const failedTarget of failedTargets) {
        console.error(`${c.error(`[${failedTarget.package.name}]`)} publish failed`);
      }
    }

    return isAllPublished ? 0 : 1;
  }

  private async prepareTargets(repo: Repository, config: Config, staged: Staged) {
    const targets: ReleaseTarget[] = [];
    const packages = await Package.loadAll(config);
    for (const pkg of packages) {
      const prefix = `[${pkg.name}]`;
      const pkgStaged = staged[pkg.name];
      if (pkgStaged == null || pkgStaged.commits.length === 0) {
        console.log(`${c.warn(prefix)} no staged changes found. skip release.`);
        continue;
      }
      const changes = Changes.fromCommits(repo, pkgStaged.commits);
      let bumpRule =
        pkgStaged.bumpRule ?? Changes.fromCommits(repo, pkgStaged.commits).getBumpRule();
      if (bumpRule == null) {
        console.log(`${c.warn(prefix)} no changes found. skip release.`);
        continue;
      }
      if (this.prerelease != null) {
        const prerelease = parsePrerelease(this.prerelease);
        bumpRule = { type: 'prerelease', data: prerelease };
      }
      pkg.bumpVersion(bumpRule);
      console.log(
        `${c.info(prefix)} ${pkg.version.toString()} -> ${c.success(pkg.nextVersion.toString())}`
      );
      for (let i = 0; i < changes.changes.length; i += 1) {
        const change = changes.changes[i]!;
        const line = i === changes.changes.length - 1 ? '└─' : '├─';
        console.log(`   ${c.dim(`${line} ${change.toString()}`)}`);
      }
      const changelog = await Changelog.load(pkg.changelog);
      targets.push({ package: pkg, changes, changelog });
    }
    return targets;
  }

  private async selectTargets(targets: ReleaseTarget[]) {
    const selected = await checkbox({
      message: 'Select release targets',
      choices: targets.map(target => {
        const prevVersion = target.package.version.toString();
        const nextVersion = target.package.nextVersion.toString();
        return {
          name: `${target.package.name} ${prevVersion} -> ${c.success(nextVersion)}`,
          value: target.package.name,
          checked: true,
        };
      }),
      loop: false,
      required: true,
    });
    const selectedTargets = targets.filter(x => selected.includes(x.package.name));
    return selectedTargets;
  }

  private async writeReleaseTarget(targets: ReleaseTarget[]) {
    for (const target of targets) {
      const actions = target.package.write();
      await runActions(actions, { name: target.package.name, dryRun: this.dryRun });

      if (target.changelog != null) {
        target.changelog.appendChanges(target.package, target.changes);
        const actions = target.changelog.write();
        await runActions(actions, { name: target.package.name, dryRun: this.dryRun });
      }
    }
  }

  private async writeRootCargoToml(targets: ReleaseTarget[]) {
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
      [
        {
          type: 'write',
          path: 'Cargo.toml',
          content: formatCargoToml(toml),
          prevContent: raw,
        },
      ],
      { dryRun: this.dryRun }
    );
    return true;
  }

  private async writeRootChangelog(config: Config, targets: ReleaseTarget[]) {
    if (config.rootChangelog == null) {
      return false;
    }
    const changelog = await Changelog.load(config.rootChangelog);
    for (const target of targets) {
      changelog.appendChanges(target.package, target.changes);
    }
    await runActions(changelog.write(), { dryRun: this.dryRun });
    return true;
  }

  private async publish(targets: ReleaseTarget[]): Promise<ReleaseTarget[]> {
    const succeedTargets: ReleaseTarget[] = [];
    for (const target of targets) {
      if (target.package.beforePublishScripts.length > 0) {
        const actions = target.package.beforePublishScripts.map(
          (script): Action => ({
            type: 'command',
            cmd: script.command,
            args: (script.args ?? []) as string[],
            path: script.cwd ?? '',
          })
        );
        const result = await runActions(actions, {
          name: target.package.name,
          dryRun: this.dryRun,
          reject: false,
        });
        if (!result.allSucceed) {
          continue;
        }
      }
      const actions = target.package.publish();
      const result = await runActions(actions, {
        name: target.package.name,
        dryRun: this.dryRun,
        failFast: false,
        reject: false,
      });
      if (result.allSucceed) {
        succeedTargets.push(target);
      }
    }
    return succeedTargets;
  }

  private gitEnsureMainBranch(repo: Repository) {
    const head = repo.head();
    const branch = head.symbolicTarget();
    if (branch !== 'refs/heads/main') {
      throw new Error('release script only run in "main" branch');
    }
  }

  private gitCommitChanges(
    repo: Repository,
    config: Config,
    targets: ReleaseTarget[],
    rootCargoChanged: boolean,
    rootChangelogChanged: boolean
  ): string | undefined {
    const message = 'release commit [skip actions]';
    if (this.dryRun) {
      console.log(`${c.info('[root]')} will commit changes: ${message}`);
      return undefined;
    }
    const pathspecs = targets.flatMap(x =>
      [...x.package.versionedFiles.map(x => x.path), x.changelog?.path, this.stagedFilepath].filter(
        isNotNil
      )
    );
    if (rootCargoChanged) {
      pathspecs.push('Cargo.toml');
    }
    if (rootChangelogChanged && config.rootChangelog != null) {
      pathspecs.push(config.rootChangelog);
    }
    const index = repo.index();
    index.addAll(pathspecs);

    const treeId = index.writeTree();
    const tree = repo.getTree(treeId);
    const parent = repo.head().target()!;
    const commitId = repo.commit(tree, message, {
      updateRef: 'refs/heads/main',
      author: GIT_SIGNATURE,
      committer: GIT_SIGNATURE,
      parents: [parent],
    });
    const commit = repo.getCommit(commitId);
    console.log(`${c.info('[root]')} commit: ${message}`);
    console.log(c.dim(`  parent: ${parent}`));
    console.log(c.dim(`  sha: ${commit.id()}`));
    console.log(c.dim(`  author name: ${commit.author().name}`));
    console.log(c.dim(`  author email: ${commit.author().email}`));
    return commitId;
  }

  private gitCreateTags(repo: Repository, commitId: string | undefined, targets: ReleaseTarget[]) {
    const commit = commitId != null ? repo.getCommit(commitId) : null;
    const targetId = commit?.id().slice(0, 7);
    for (const target of targets) {
      const prefix = `[${target.package.name}]`;
      const tag = target.package.nextVersionedGitTag;
      if (commit == null || this.dryRun) {
        console.log(`${c.info(prefix)} will create tag (${targetId}): ${tag.tagName}`);
        continue;
      }
      const tagId = repo.createTag(tag.tagName, commit.asObject(), tag.tagName, {
        tagger: GIT_SIGNATURE,
      });
      const gitTag = repo.getTag(tagId);
      console.log(`${c.info('[root]')} tag: ${gitTag.name()}`);
      console.log(c.dim(`  sha: ${gitTag.id()}`));
      console.log(c.dim(`  message: ${gitTag.message()}`));
      console.log(c.dim(`  tagger name: ${gitTag.tagger()?.name}`));
      console.log(c.dim(`  tagger email: ${gitTag.tagger()?.email}`));
    }
  }

  private async gitPush(repo: Repository, targets: ReleaseTarget[]) {
    const remote = repo.getRemote('origin');
    const refspecs = [
      'refs/heads/main:refs/heads/main',
      ...targets
        .map(x => x.package.nextVersionedGitTag.tagRef)
        .map(tagRef => `${tagRef}:${tagRef}`),
    ];
    const logRefspecs = () => {
      for (const refspec of refspecs) {
        const [src, dest] = refspec.split(':');
        console.log(c.dim(`  - ${src} -> ${dest}`));
      }
    };
    if (this.dryRun) {
      console.log(`${c.info('[root]')} will push to: ${remote.url()}`);
      logRefspecs();
      return;
    }
    if (this.githubToken == null) {
      console.log(`${c.warn('[root]')} no github token found. skip push.`);
      return;
    }
    await remote.push(refspecs, {
      credential: {
        type: 'Plain',
        password: this.githubToken,
      },
    });
    console.log(`${c.success('[root]')} push changes to remote: ${remote.url()}`);
    logRefspecs();
  }

  private async createGitHubReleases(config: Config, targets: ReleaseTarget[]) {
    const { github } = config;
    const GitHubClient = Octokit.plugin(retry);
    const client = new GitHubClient({
      auth: this.githubToken,
      userAgent: 'webview-bundle',
    });
    for (const target of targets) {
      const payload = {
        tag_name: target.package.nextVersionedGitTag.tagName,
        name: `${target.package.name} v${target.package.nextVersion.toString()}`,
        body: target.changelog?.extractChanges(target.package) ?? undefined,
      };
      if (this.dryRun) {
        console.log(
          `${c.info('[root]')} will create github release: ${github.repo.owner}/${github.repo.name}`
        );
        const payloadStr = JSON.stringify(payload, null, 2);
        for (const line of payloadStr.split('\n')) {
          console.log(`  ${c.dim(line)}`);
        }
        continue;
      }
      if (this.githubToken == null) {
        console.log(`${c.warn('[root]')} no github token found. skip creating github release.`);
        return;
      }
      const release = await client.rest.repos.createRelease({
        owner: github.repo.owner,
        repo: github.repo.name,
        ...payload,
      });
      console.log(`${c.success('[root]')} create github release: ${release.data.tag_name}`);
      console.log(`  ${c.dim(`id: ${release.data.id}`)}`);
      console.log(`  ${c.dim(`html_url: ${release.data.html_url}`)}`);
    }
  }

  private async updateStaged(targets: ReleaseTarget[], staged: Staged) {
    const releasedPackages = targets.map(x => x.package.name);
    const updatedStaged = omit(staged, releasedPackages);
    if (Object.keys(updatedStaged).length === 0) {
      await removeStaged(this.stagedFilepath);
    } else {
      await saveStaged(this.stagedFilepath, updatedStaged);
    }
  }
}
