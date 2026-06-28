import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import { type Commit, type Credential, type Index, openRepository, type Repository } from 'es-git';
import * as t from 'typanion';
import { runCommand } from '../child_process.ts';
import { ColorModeOption, c, setColorMode } from '../console.ts';
import { GITHUB_REPO, ROOT_DIR } from '../consts.ts';
import { createGitHubClient, type GitHubClient } from '../github.ts';
import {
  classifyConflict,
  evaluateStatusChecks,
  indexEntryStage,
  LOCKFILE,
} from '../lockfile-merge.ts';

type ReactionContent = 'eyes' | 'rocket' | 'confused' | '-1';

/**
 * Yarn toolchain files. `yarn install` executes the checked-out Yarn release and any local plugins,
 * so a PR that changes these could run arbitrary code under the bot token — the bot refuses those.
 */
const TOOLCHAIN_PATHS = ['.yarnrc.yml', '.yarn/releases', '.yarn/plugins'];

/**
 * Auto-resolve a pull request whose only merge conflict with its base branch is `yarn.lock`.
 *
 * Invoked from the `resolve-lockfile` workflow when a maintainer comments the trigger command on a
 * PR. The algorithm mirrors what a human would do by hand:
 *
 *   1. Verify the PR is open, same-repo (forks are out of scope), and its head's status checks are
 *      green — i.e. it is only the conflict that blocks the merge.
 *   2. Merge the base branch into the head in-memory (via es-git/libgit2) and inspect the index for
 *      what conflicts.
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
  #repo: Repository | null = null;

  async execute(): Promise<number> {
    setColorMode(this.colorMode);
    if (this.githubToken == null || this.githubToken.length === 0) {
      console.error(c.error('a GitHub token is required (--github-token or $GITHUB_TOKEN)'));
      return 1;
    }
    this.#client = createGitHubClient(this.githubToken);

    // The workflow already reacts 👀 to the triggering comment before this process starts (so the
    // commenter gets instant feedback during checkout/setup); here we only add the outcome
    // reaction — 🚀 on success, 😕 when there's nothing to do, 👎 on failure.
    try {
      return await this.run();
    } catch (error) {
      // Keep the detailed error (which may include git/command stderr) in the workflow logs only,
      // not in the public comment.
      const message = error instanceof Error ? error.message : String(error);
      console.error(c.error(`resolve-lockfile failed: ${message}`));
      this.safeAbortMerge();
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

    const token = this.githubToken;
    if (token == null || token.length === 0) {
      throw new Error('a GitHub token is required');
    }
    const credential: Credential = { type: 'Plain', username: 'x-access-token', password: token };
    const gitRepo = await openRepository(ROOT_DIR);
    this.#repo = gitRepo;
    const origin = gitRepo.getRemote('origin');

    // Bring both branches into the local clone (the workflow checks out the default branch only).
    await origin.fetch(
      [
        `+refs/heads/${base}:refs/remotes/origin/${base}`,
        `+refs/heads/${head}:refs/remotes/origin/${head}`,
      ],
      { fetch: { credential } }
    );
    const headCommit = gitRepo.getCommit(gitRepo.revparseSingle(`refs/remotes/origin/${head}`));
    const baseCommit = gitRepo.getCommit(gitRepo.revparseSingle(`refs/remotes/origin/${base}`));

    // Check the PR head out into the working tree so the merge and `yarn install` see its files.
    gitRepo.createBranch(head, headCommit, { force: true });
    gitRepo.setHead(`refs/heads/${head}`);
    gitRepo.checkoutHead({ force: true });

    // Security gate: `yarn install` executes the checked-out Yarn release and any local plugins
    // (corepack honors the merged `yarnPath`; `.yarnrc.yml` declares plugins). If this PR changed
    // the toolchain, that code would run under the bot token — refuse and defer to a human.
    const toolchainChanged = this.changedPaths(gitRepo, baseCommit, headCommit, TOOLCHAIN_PATHS);
    if (toolchainChanged.length > 0) {
      const list = toolchainChanged.map(file => `- \`${file}\``).join('\n');
      return this.bail(
        `This PR modifies the Yarn toolchain, which the bot must not execute. Resolve it manually:\n${list}`
      );
    }

    // Merge the base branch into HEAD; conflicts are written to the index rather than thrown.
    gitRepo.merge([gitRepo.getAnnotatedCommit(baseCommit)]);
    const index = gitRepo.index();
    const classification = classifyConflict(this.conflictedPaths(index));

    if (classification.type === 'none') {
      gitRepo.cleanupState();
      return this.bail('There is no conflict between this PR and its base branch.');
    }
    if (classification.type === 'other') {
      gitRepo.cleanupState();
      const list = classification.files.map(file => `- \`${file}\``).join('\n');
      return this.bail(
        `Conflicts are not limited to \`${LOCKFILE}\`, so this needs a human:\n${list}`
      );
    }

    // lockfile-only: write the base branch's lockfile as the starting point, then regenerate it so
    // it satisfies the merged `package.json` set.
    console.log(`${c.info('[resolve-lockfile]')} regenerating ${LOCKFILE} …`);
    const baseLock = baseCommit.tree().getPath(LOCKFILE);
    if (baseLock == null) {
      throw new Error(`${LOCKFILE} does not exist on ${base}`);
    }
    await fs.writeFile(
      path.join(ROOT_DIR, LOCKFILE),
      baseLock.toObject(gitRepo).peelToBlob().content()
    );
    const install = await runCommand('yarn', ['install', '--mode', 'update-lockfile'], {
      cwd: ROOT_DIR,
      prefix: `${c.dim('[yarn]')} `,
    });
    if (install.exitCode !== 0) {
      throw new Error(`yarn install failed with exit code ${install.exitCode}`);
    }

    // Stage the regenerated lockfile (clears its conflict) and build the merge commit from the index.
    index.addPath(LOCKFILE);
    index.write();
    if (index.hasConflicts()) {
      throw new Error('conflicts remain after regenerating the lockfile');
    }
    const tree = gitRepo.getTree(index.writeTree());
    const signature = { name: this.committerName, email: this.committerEmail };
    const mergeSha = gitRepo.commit(
      tree,
      `chore: merge ${base} into ${head} and regenerate ${LOCKFILE}`,
      {
        updateRef: `refs/heads/${head}`,
        author: signature,
        committer: signature,
        parents: [headCommit.id(), baseCommit.id()],
      }
    );
    gitRepo.cleanupState();

    if (this.dryRun) {
      console.log(`${c.warn('[dry-run]')} would push ${mergeSha} to ${head}`);
      await this.comment(
        `🧪 [dry-run] Resolved \`${LOCKFILE}\` locally (\`${mergeSha.slice(0, 9)}\`) but did not push.`
      );
      return 0;
    }

    await origin.push([`refs/heads/${head}:refs/heads/${head}`], { credential });
    console.log(c.success(`pushed ${mergeSha} to ${head}`));
    await this.react('rocket');
    await this.comment(
      `✅ Resolved the \`${LOCKFILE}\` conflict by merging \`${base}\` and regenerating the lockfile ` +
        `from \`${base}\` (\`${mergeSha.slice(0, 9)}\`).`
    );
    return 0;
  }

  /** Distinct paths left in conflict (index stage > 0) after a merge. */
  private conflictedPaths(index: Index): string[] {
    const paths = new Set<string>();
    const entries = index.entries();
    for (let next = entries.next(); next.done !== true; next = entries.next()) {
      if (indexEntryStage(next.value.flags) !== 0) {
        paths.add(next.value.path.toString('utf8'));
      }
    }
    return [...paths];
  }

  /** Paths under `pathspecs` that `to` changed relative to its merge-base with `from`. */
  private changedPaths(repo: Repository, from: Commit, to: Commit, pathspecs: string[]): string[] {
    const mergeBase = repo.getCommit(repo.getMergeBase(from.id(), to.id()));
    const diff = repo.diffTreeToTree(mergeBase.tree(), to.tree(), { pathspecs });
    const changed: string[] = [];
    const deltas = diff.deltas();
    for (let next = deltas.next(); next.done !== true; next = deltas.next()) {
      const file = next.value.newFile().path() ?? next.value.oldFile().path();
      if (file != null) {
        changed.push(file);
      }
    }
    return changed;
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

  private safeAbortMerge(): void {
    // Clear any in-progress merge state; ignore if no merge is in progress or the repo isn't open.
    try {
      this.#repo?.cleanupState();
    } catch {
      // nothing to clean up
    }
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
