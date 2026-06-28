import { Command, Option } from 'clipanion';
import { execa } from 'execa';
import * as t from 'typanion';
import { runCommand } from '../child_process.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GITHUB_REPO, ROOT_DIR } from '../consts.ts';
import { createGitHubClient, type GitHubClient } from '../github.ts';
import {
  classifyConflict,
  evaluateStatusChecks,
  LOCKFILE,
  parseConflictedFiles,
} from '../lockfile-merge.ts';

interface GitResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

/** Run a git subcommand in the repo root, capturing output. Never rejects — inspect `exitCode`. */
async function git(args: string[]): Promise<GitResult> {
  const res = await (execa as any)('git', args, { cwd: ROOT_DIR, reject: false });
  return {
    exitCode: typeof res.exitCode === 'number' ? res.exitCode : 1,
    stdout: typeof res.stdout === 'string' ? res.stdout : '',
    stderr: typeof res.stderr === 'string' ? res.stderr : '',
  };
}

/** Run git expecting success; throws with stderr on a non-zero exit. */
async function gitOk(args: string[]): Promise<string> {
  const res = await git(args);
  if (res.exitCode !== 0) {
    throw new Error(`git ${args.join(' ')} failed (${res.exitCode}): ${res.stderr.trim()}`);
  }
  return res.stdout.trim();
}

type ReactionContent = 'eyes' | 'rocket' | 'confused' | '-1';

/**
 * Auto-resolve a pull request whose only merge conflict with its base branch is `yarn.lock`.
 *
 * Invoked from the `resolve-lockfile` workflow when a maintainer comments the trigger command on a
 * PR. The algorithm mirrors what a human would do by hand:
 *
 *   1. Verify the PR is open, same-repo (forks are out of scope), and its head's status checks are
 *      green — i.e. it is only the conflict that blocks the merge.
 *   2. Merge the base branch into the head locally (`--no-commit`) and look at what conflicts.
 *   3. If `yarn.lock` is the *only* conflict, take the base branch's `yarn.lock` and run
 *      `yarn install` to regenerate it against the merged `package.json` set, then commit the merge
 *      and push it back to the head branch. Any other conflict aborts and asks for a human.
 *
 * The bot never merges the PR itself: pushing re-runs CI, and a human merges once it is green.
 */
export class ResolveLockfileCommand extends Command {
  static paths = [['resolve-lockfile']];

  readonly pr = Option.String('--pr', {
    required: true,
    description: 'Pull request number to resolve',
    validator: t.cascade(t.isNumber(), [t.isInteger(), t.isPositive()]),
  });
  readonly githubToken = Option.String('--github-token', { required: false, env: 'GITHUB_TOKEN' });
  /** The id of the triggering comment; when set, the bot reacts to it to signal progress. */
  readonly commentId = Option.String('--comment-id', {
    required: false,
    env: 'COMMENT_ID',
    validator: t.cascade(t.isNumber(), [t.isInteger()]),
  });
  /**
   * The login of the user who triggered the command. When set, the bot verifies they actually have
   * write access before acting — `author_association` (checked in the workflow) is only a coarse
   * pre-filter and includes read-only org members/collaborators.
   */
  readonly commentUser = Option.String('--comment-user', { required: false, env: 'COMMENT_USER' });
  /**
   * Whether the push will re-trigger CI. A push authenticated with the default `GITHUB_TOKEN` does
   * NOT start new workflow runs; the workflow sets this from whether a dedicated bot token is used,
   * so the success comment can tell the maintainer the truth about CI re-running.
   */
  readonly ciWillRerun = Option.String('--ci-will-rerun', 'false', { env: 'CI_WILL_RERUN' });
  readonly committerName = Option.String('--committer-name', 'github-actions[bot]');
  readonly committerEmail = Option.String(
    '--committer-email',
    '41898282+github-actions[bot]@users.noreply.github.com'
  );
  /** Skip the status-check gate (for local testing / manual override). */
  readonly skipChecks = Option.Boolean('--skip-checks', false);
  /** Resolve and commit locally but do not push. */
  readonly dryRun = Option.Boolean('--dry-run', false);
  readonly colorMode = ColorModeOption;

  #client!: GitHubClient;

  async execute(): Promise<number> {
    setColorMode(this.colorMode);
    if (this.githubToken == null || this.githubToken.length === 0) {
      console.error(c.error('a GitHub token is required (--github-token or $GITHUB_TOKEN)'));
      return 1;
    }
    this.#client = createGitHubClient(this.githubToken);

    await this.react('eyes');
    try {
      return await this.run();
    } catch (error) {
      // Keep the detailed error (which may include git/command stderr) in the workflow logs only,
      // not in the public comment.
      const message = error instanceof Error ? error.message : String(error);
      console.error(c.error(`resolve-lockfile failed: ${message}`));
      await this.safeAbortMerge();
      await this.react('-1');
      await this.comment(
        `❌ Failed to auto-resolve \`${LOCKFILE}\`. The merge was aborted; please resolve it manually. See the workflow run logs for details.`
      );
      return 1;
    }
  }

  async run(): Promise<number> {
    const { owner, name: repo } = GITHUB_REPO;

    // Authorize the commenter by *actual* repository permission, not `author_association`: a
    // read-only org member or triage collaborator also reports MEMBER/COLLABORATOR.
    if (this.commentUser != null && this.commentUser.length > 0) {
      const { data: perm } = await this.#client.rest.repos.getCollaboratorPermissionLevel({
        owner,
        repo,
        username: this.commentUser,
      });
      // `permission` collapses to admin|write|read|none (maintain→write, triage→read).
      if (perm.permission !== 'admin' && perm.permission !== 'write') {
        return this.bail(`@${this.commentUser} does not have write access to this repository.`);
      }
    }

    const pull = (await this.#client.rest.pulls.get({ owner, repo, pull_number: this.pr })).data;

    if (pull.state !== 'open') {
      return this.bail(`This PR is ${pull.state}, not open.`);
    }
    // Forks are out of scope: GITHUB_TOKEN cannot push to a fork's branch, and accepting
    // fork-authored pushes is a separate security decision.
    if (pull.head.repo == null || pull.head.repo.full_name !== `${owner}/${repo}`) {
      return this.bail(
        'This PR comes from a fork. Only same-repository branches are supported for now.'
      );
    }

    const head = pull.head.ref;
    const base = pull.base.ref;
    const headSha = pull.head.sha;
    console.log(`${c.info('[resolve-lockfile]')} PR #${this.pr}: ${head} ← ${base} (@${headSha})`);

    if (!this.skipChecks) {
      const checks = await this.evaluateChecks(headSha);
      if (!checks.passed) {
        return this.bail(`Status checks are not green yet — ${checks.reason}.`);
      }
    }

    // Bring both branches into the local clone (the workflow checks out the default branch only).
    await gitOk(['config', 'user.name', this.committerName]);
    await gitOk(['config', 'user.email', this.committerEmail]);
    await gitOk([
      'fetch',
      '--no-tags',
      'origin',
      `+refs/heads/${base}:refs/remotes/origin/${base}`,
      `+refs/heads/${head}:refs/remotes/origin/${head}`,
    ]);
    await gitOk(['checkout', '-B', head, `refs/remotes/origin/${head}`]);

    // Security gate: running `yarn install` executes the *checked-out* Yarn release and any local
    // plugins (corepack honors the merged `yarnPath`; `.yarnrc.yml` declares plugins). If this PR
    // changed the toolchain, that code would run under the bot token — so refuse and defer to a
    // human rather than execute a PR-supplied binary/plugin.
    const mergeBase = await gitOk(['merge-base', `refs/remotes/origin/${base}`, 'HEAD']);
    const toolchainChanged = parseConflictedFiles(
      await gitOk([
        'diff',
        '--name-only',
        mergeBase,
        'HEAD',
        '--',
        '.yarnrc.yml',
        '.yarn/releases',
        '.yarn/plugins',
      ])
    );
    if (toolchainChanged.length > 0) {
      const list = toolchainChanged.map(file => `- \`${file}\``).join('\n');
      return this.bail(
        `This PR modifies the Yarn toolchain, which the bot must not execute. Resolve it manually:\n${list}`
      );
    }

    // Attempt the merge without committing; conflicts make this exit non-zero, which is expected.
    await git(['merge', '--no-ff', '--no-commit', `refs/remotes/origin/${base}`]);
    const conflicted = parseConflictedFiles(
      await gitOk(['diff', '--name-only', '--diff-filter=U'])
    );
    const classification = classifyConflict(conflicted);

    if (classification.type === 'none') {
      await this.safeAbortMerge();
      return this.bail('There is no conflict between this PR and its base branch.');
    }
    if (classification.type === 'other') {
      await this.safeAbortMerge();
      const list = classification.files.map(file => `- \`${file}\``).join('\n');
      return this.bail(
        `Conflicts are not limited to \`${LOCKFILE}\`, so this needs a human:\n${list}`
      );
    }

    // lockfile-only: take the base branch's lockfile as the starting point, then regenerate it so
    // it satisfies the merged `package.json` set.
    console.log(`${c.info('[resolve-lockfile]')} regenerating ${LOCKFILE} …`);
    await gitOk(['checkout', `refs/remotes/origin/${base}`, '--', LOCKFILE]);
    const install = await runCommand('yarn', ['install', '--mode', 'update-lockfile'], {
      cwd: ROOT_DIR,
      prefix: `${c.dim('[yarn]')} `,
    });
    if (install.exitCode !== 0) {
      throw new Error(`yarn install failed with exit code ${install.exitCode}`);
    }
    await gitOk(['add', '--', LOCKFILE]);

    // Safety net: after resolving, no path may be left unmerged (every conflict is at stage 0).
    const stillConflicted = parseConflictedFiles(
      await gitOk(['diff', '--name-only', '--diff-filter=U'])
    );
    if (stillConflicted.length > 0) {
      throw new Error(`unexpected unmerged paths remain: ${stillConflicted.join(', ')}`);
    }

    const message = `chore: merge ${base} into ${head} and regenerate ${LOCKFILE}`;
    await gitOk(['commit', '--no-edit', '-m', message]);
    const mergeSha = await gitOk(['rev-parse', 'HEAD']);

    if (this.dryRun) {
      console.log(`${c.warn('[dry-run]')} would push ${mergeSha} to ${head}`);
      await this.comment(
        `🧪 [dry-run] Resolved \`${LOCKFILE}\` locally (\`${mergeSha.slice(0, 9)}\`) but did not push.`
      );
      return 0;
    }

    await gitOk(['push', 'origin', `HEAD:refs/heads/${head}`]);
    console.log(c.success(`pushed ${mergeSha} to ${head}`));
    await this.react('rocket');
    // A push made with the default GITHUB_TOKEN does not start new workflow runs, so be honest
    // about whether CI will actually re-run on the merge commit.
    const ciNote =
      this.ciWillRerun === 'true'
        ? 'CI will re-run on the new commit — merge once it is green.'
        : '⚠️ CI will **not** re-run automatically (pushed with the default token). Re-trigger CI ' +
          'on the new commit (e.g. push an empty commit, or close/reopen the PR) before merging.';
    await this.comment(
      `✅ Resolved the \`${LOCKFILE}\` conflict by merging \`${base}\` and regenerating the lockfile ` +
        `from \`${base}\` (\`${mergeSha.slice(0, 9)}\`). ${ciNote}`
    );
    return 0;
  }

  /** Read the head's check-runs + commit-status rollup and decide whether they are green. */
  private async evaluateChecks(headSha: string) {
    const { owner, name: repo } = GITHUB_REPO;
    const [checkRuns, combined] = await Promise.all([
      this.#client.paginate(this.#client.rest.checks.listForRef, {
        owner,
        repo,
        ref: headSha,
        per_page: 100,
      }),
      this.#client.rest.repos.getCombinedStatusForRef({ owner, repo, ref: headSha }),
    ]);
    return evaluateStatusChecks({
      checkRuns: checkRuns.map(run => ({
        name: run.name,
        status: run.status,
        conclusion: run.conclusion,
      })),
      combinedStatusState: combined.data.state as 'success' | 'pending' | 'failure',
      combinedStatusCount: combined.data.total_count,
      // The resolver runs from `issue_comment`, not as a PR check, so it won't appear here — but
      // guard against the name just in case the repo wires it up as a check later.
      ignoreCheckNames: ['resolve-lockfile'],
    });
  }

  /** Report a non-actionable situation back to the PR and exit cleanly (not an error). */
  private async bail(reason: string): Promise<number> {
    console.log(`${c.warn('[resolve-lockfile]')} ${reason}`);
    await this.react('confused');
    await this.comment(`ℹ️ Nothing to do: ${reason}`);
    return 0;
  }

  private async safeAbortMerge(): Promise<void> {
    // Only abort if a merge is actually in progress; ignore "no merge to abort".
    await git(['merge', '--abort']);
  }

  private async comment(body: string): Promise<void> {
    try {
      await this.#client.rest.issues.createComment({
        owner: GITHUB_REPO.owner,
        repo: GITHUB_REPO.name,
        issue_number: this.pr,
        body,
      });
    } catch (error) {
      console.warn(
        c.warn(`could not post comment: ${error instanceof Error ? error.message : error}`)
      );
    }
  }

  private async react(content: ReactionContent): Promise<void> {
    if (this.commentId == null) {
      return;
    }
    try {
      await this.#client.rest.reactions.createForIssueComment({
        owner: GITHUB_REPO.owner,
        repo: GITHUB_REPO.name,
        comment_id: this.commentId,
        content,
      });
    } catch {
      // Reactions are cosmetic; never fail the run over them.
    }
  }
}
