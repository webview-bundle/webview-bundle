import { checkbox, select } from '@inquirer/prompts';
import { Command, Option } from 'clipanion';
import { type Commit, openRepository, type Repository } from 'es-git';
import { Changelog } from '../changelog.ts';
import { Change, Changes } from '../changes.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GIT_SIGNATURE, RELEASE_COMMIT_PREFIX, ROOT_DIR } from '../consts.ts';
import { createPullRequest, findOpenPullRequest, updatePullRequest } from '../github.ts';
import type { Package } from '../package.ts';
import {
  logTarget,
  planRelease,
  type ReleasePlan,
  type ReleaseTarget,
  releaseBranchName,
  releasePathspecs,
  writeReleaseTargets,
} from '../releasing.ts';
import type { BumpRule } from '../version.ts';

type BumpChoice = BumpRule | 'skip';

interface ChangeChoice {
  name: string;
  value: string;
  checked: boolean;
}

/**
 * Prepare a release locally (run by a maintainer).
 *
 * Determines, per package, the commits since its last tag (`<name>/<version>`) that touched the
 * package, lets the maintainer pick which ones go in the changelog, and propagates through the
 * `package.json`/`Cargo.toml` dependency graph so dependents are released too. Version bumps +
 * changelogs are committed to a content-addressed `release/<hash>` branch and opened as a PR;
 * merging that PR is what triggers the actual publish (`xtask release`).
 */
export class PrepareReleaseCommand extends Command {
  static paths = [['prepare-release']];

  readonly base = Option.String('--base', 'main');
  readonly branch = Option.String('--branch', { required: false });
  // Open the PR as a draft by default (use `--no-draft` for a ready-for-review PR). Only applies
  // when the PR is first created; refreshes leave the draft state as-is.
  readonly draft = Option.Boolean('--draft', true);
  readonly dryRun = Option.Boolean('--dry-run', false);
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    const repo = await openRepository(ROOT_DIR);
    await this.fetchTags(repo);

    const plan = await planRelease(repo);
    if (plan.candidates.length === 0) {
      console.log(`${c.warn('[root]')} no changes since last release. nothing to prepare.`);
      return 0;
    }

    const targets = await this.selectTargets(plan);
    if (targets.length === 0) {
      console.log(`${c.warn('[root]')} no release targets selected.`);
      return 0;
    }
    for (const target of targets) {
      logTarget(target);
    }

    await this.openReleasePr(repo, targets);
    return 0;
  }

  /** Fetch tags (so "commits since last tag" is accurate) and make sure `HEAD` exists. */
  private async fetchTags(repo: Repository): Promise<void> {
    const remote = repo.getRemote('origin');
    await remote.fetch([], {
      fetch: { downloadTags: 'All', credential: { type: 'SSHKeyFromAgent' } },
    });
    if (repo.head().target() == null) {
      throw new Error('cannot find git `HEAD` target');
    }
  }

  /** Walk the candidates in order, prompting the maintainer for each, and collect the targets. */
  private async selectTargets(plan: ReleasePlan): Promise<ReleaseTarget[]> {
    const targets: ReleaseTarget[] = [];
    for (const pkg of plan.candidates) {
      const target = await this.selectTarget(plan, pkg, targets);
      if (target != null) {
        targets.push(target);
      }
    }
    return targets;
  }

  /** Prompt for a single package: which changes to record and how to bump it (or skip it). */
  private async selectTarget(
    plan: ReleasePlan,
    pkg: Package,
    decided: ReleaseTarget[]
  ): Promise<ReleaseTarget | null> {
    const prefix = c.info(`[${pkg.name}]`);
    const directCommits = plan.directChanges.get(pkg) ?? [];
    // Dependencies already queued for release (topo order ⇒ decided first), reused as changes.
    const changedDeps = plan.graph
      .dependenciesOf(pkg)
      .filter(dep => decided.some(target => target.package === dep));

    const { choices, summaryByCommitId } = this.changeChoices(directCommits, changedDeps, decided);
    if (choices.length === 0 && changedDeps.length === 0) {
      console.log(`${prefix} nothing to release. skip.`);
      return null;
    }

    const selectedCommitIds = choices.length > 0 ? await this.promptChanges(prefix, choices) : [];
    const reason =
      directCommits.length === 0
        ? `dependency update: ${changedDeps.map(dep => dep.name).join(', ')}`
        : 'direct changes';
    const bump = await this.promptBump(prefix, reason);
    if (bump === 'skip') {
      console.log(`${prefix} skipped.`);
      return null;
    }

    pkg.bumpVersion(bump);
    const changes = this.buildChanges(selectedCommitIds, summaryByCommitId, changedDeps);
    const changelog = await Changelog.load(pkg.changelog).catch(() => null);
    return { package: pkg, changes, changelog };
  }

  /**
   * The changes a package can include: its own commits (checked by default) plus the curated
   * commits of its changed dependencies (opt-in, marked `via <dep>`). Returns the checkbox choices
   * and a sha→summary lookup for turning the selection back into changelog entries.
   */
  private changeChoices(
    directCommits: Commit[],
    changedDeps: Package[],
    decided: ReleaseTarget[]
  ): { choices: ChangeChoice[]; summaryByCommitId: Map<string, string> } {
    const summaryByCommitId = new Map<string, string>();
    const choices: ChangeChoice[] = [];
    const add = (sha: string, summary: string, label: string, checked: boolean): void => {
      if (summaryByCommitId.has(sha)) {
        return;
      }
      summaryByCommitId.set(sha, summary);
      choices.push({ name: label, value: sha, checked });
    };

    for (const commit of directCommits) {
      const sha = commit.id();
      const summary = commit.summary() ?? '';
      add(sha, summary, `[${sha.slice(0, 7)}] ${summary}`, true);
    }
    for (const dep of changedDeps) {
      const depTarget = decided.find(target => target.package === dep)!;
      for (const change of depTarget.changes.changes) {
        if (change.sha == null) {
          continue;
        }
        const label = `[${change.sha.slice(0, 7)}] ${change.summary} ${c.dim(`(via ${dep.name})`)}`;
        add(change.sha, change.summary, label, false);
      }
    }
    return { choices, summaryByCommitId };
  }

  private promptChanges(prefix: string, choices: ChangeChoice[]): Promise<string[]> {
    return checkbox({
      message: `${prefix} Select changes to include in the release`,
      choices,
      loop: false,
    });
  }

  private promptBump(prefix: string, reason: string): Promise<BumpChoice> {
    return select<BumpChoice>({
      message: `${prefix} Select version bump (${reason})`,
      choices: [
        { name: 'patch', value: 'patch' },
        { name: 'minor', value: 'minor' },
        { name: 'major', value: 'major' },
        { name: 'skip (do not release)', value: 'skip' },
      ],
      default: 'patch',
      loop: false,
    });
  }

  /** Turn the selected commits (+ an automatic dependency-bump note) into changelog entries. */
  private buildChanges(
    commitIds: string[],
    summaryByCommitId: Map<string, string>,
    changedDeps: Package[]
  ): Changes {
    const changes = commitIds.map(
      commitId => new Change(summaryByCommitId.get(commitId)!, commitId)
    );

    if (changedDeps.length > 0) {
      const note = changedDeps
        .flatMap(dep => dep.versionFileNames.map(name => `${name}@${dep.nextVersion.toString()}`))
        .join(', ');
      changes.push(new Change(`update dependencies: ${note}`));
    }

    return new Changes(changes);
  }

  /** Write the bumps + changelogs, commit them to the release branch, push, and open/update the PR. */
  private async openReleasePr(repo: Repository, targets: ReleaseTarget[]): Promise<void> {
    await writeReleaseTargets(targets, { dryRun: this.dryRun });
    const headBranch = this.branch ?? releaseBranchName(targets);
    this.commitToBranch(repo, targets, headBranch);
    await this.pushBranch(repo, headBranch);
    await this.upsertPullRequest(targets, headBranch);
  }

  private commitToBranch(repo: Repository, targets: ReleaseTarget[], headBranch: string): void {
    const message = this.prTitle(targets);
    if (this.dryRun) {
      console.log(`${c.info('[root]')} will commit to "${headBranch}": ${message}`);
      return;
    }
    const index = repo.index();
    index.addAll(releasePathspecs(targets));
    const treeId = index.writeTree();
    const tree = repo.getTree(treeId);
    // Parent is the base branch tip, so the release branch is always "base + one release commit".
    const parent = repo.head().target()!;
    const commitId = repo.commit(tree, message, {
      updateRef: `refs/heads/${headBranch}`,
      author: GIT_SIGNATURE,
      committer: GIT_SIGNATURE,
      parents: [parent],
    });
    console.log(
      `${c.success('[root]')} committed release changes to "${headBranch}" (${commitId.slice(0, 7)})`
    );
  }

  private async pushBranch(repo: Repository, headBranch: string): Promise<void> {
    if (this.dryRun) {
      console.log(`${c.info('[root]')} will push "${headBranch}"`);
      return;
    }
    const remote = repo.getRemote('origin');
    // Force so the branch is reset to "base + release commit" on every refresh.
    await remote.push([`+refs/heads/${headBranch}:refs/heads/${headBranch}`], {
      credential: { type: 'SSHKeyFromAgent' },
    });
    console.log(`${c.success('[root]')} pushed "${headBranch}"`);
  }

  /** Open or update the Release PR via the `gh` CLI (uses the maintainer's existing gh auth). */
  private async upsertPullRequest(targets: ReleaseTarget[], headBranch: string): Promise<void> {
    const title = this.prTitle(targets);
    const body = this.prBody(targets);
    if (this.dryRun) {
      console.log(`${c.info('[root]')} release PR title: ${title}${this.draft ? ' (draft)' : ''}`);
      for (const line of body.split('\n')) {
        console.log(`  ${c.dim(line)}`);
      }
      return;
    }

    const number = await findOpenPullRequest(headBranch);
    if (number != null) {
      await updatePullRequest(number, { title, body });
      console.log(`${c.success('[root]')} updated release PR #${number}`);
    } else {
      const url = await createPullRequest({
        base: this.base,
        head: headBranch,
        title,
        body,
        draft: this.draft,
      });
      console.log(`${c.success('[root]')} opened release PR: ${url}`);
    }
  }

  private prTitle(targets: ReleaseTarget[]): string {
    if (targets.length === 1) {
      const target = targets[0]!;
      return `${RELEASE_COMMIT_PREFIX} ${target.package.name} v${target.package.nextVersion.toString()}`;
    }
    return `${RELEASE_COMMIT_PREFIX} ${targets.length} packages`;
  }

  private prBody(targets: ReleaseTarget[]): string {
    const lines = [
      'Generated by `xtask prepare-release`. Merging this PR publishes the packages below.',
      '',
    ];
    for (const target of targets) {
      lines.push(`### ${target.package.name} v${target.package.nextVersion.toString()}`);
      for (const change of target.changes.changes) {
        lines.push(`- ${change.toString()}`);
      }
      lines.push('');
    }
    return lines.join('\n');
  }
}
