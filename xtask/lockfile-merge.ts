/** The lockfile this bot knows how to auto-resolve. */
export const LOCKFILE = 'yarn.lock';

/** Slash command that triggers the resolver from a PR comment. */
export const TRIGGER_COMMAND = '/merge-lockfile';

/**
 * Outcome of classifying the files left unmerged after attempting to merge the base branch into a
 * PR head branch.
 *
 * - `none` — the merge had no conflicts (base already merges cleanly); nothing to do.
 * - `lockfile-only` — the only conflict is `yarn.lock`; safe to auto-resolve by regenerating it.
 * - `other` — at least one non-lockfile path conflicts; a human has to resolve it.
 */
export type ConflictClassification =
  | { type: 'none' }
  | { type: 'lockfile-only' }
  | { type: 'other'; files: string[] };

/**
 * Classify the set of unmerged (conflicted) paths reported by `git diff --diff-filter=U`.
 *
 * The bot only auto-resolves when `yarn.lock` is the *sole* conflict: a conflict anywhere else
 * means the merge carries real source changes that must be reviewed by a human, so we bail out
 * rather than guess.
 */
export function classifyConflict(conflictedFiles: readonly string[]): ConflictClassification {
  const files = conflictedFiles.map(file => file.trim()).filter(file => file.length > 0);
  if (files.length === 0) {
    return { type: 'none' };
  }
  if (files.length === 1 && files[0] === LOCKFILE) {
    return { type: 'lockfile-only' };
  }
  return { type: 'other', files };
}

/**
 * Decode the merge stage from a libgit2 index entry's `flags`. Stage 0 is a normally-staged file;
 * stages 1/2/3 (ancestor/ours/theirs) mark a path still in conflict. The stage lives in bits 12-13
 * of `flags` (mask `0x3000`).
 */
export function indexEntryStage(flags: number): number {
  return (flags >> 12) & 0x3;
}

export interface CheckRunLike {
  name: string;
  /** GitHub check-run status, e.g. `queued` | `in_progress` | `completed`. */
  status: string;
  /** GitHub check-run conclusion, e.g. `success` | `failure` | `neutral` | `skipped`, or null. */
  conclusion: string | null;
}

/** A check-run conclusion that does not block merging. */
const PASSING_CONCLUSIONS = new Set(['success', 'neutral', 'skipped']);

export type StatusChecksResult = { passed: true } | { passed: false; reason: string };

/**
 * Decide whether a PR head's status checks are "green enough" to act on.
 *
 * A check blocks if it is still running (`status !== 'completed'`) or completed with a conclusion
 * outside {success, neutral, skipped}. `combinedStatusState` is the legacy commit-status rollup
 * (`success` | `pending` | `failure`); it only blocks when there is at least one such status.
 *
 * `ignoreCheckNames` lets the caller exclude this bot's own check-run (if it ever surfaces as one)
 * so the resolver never waits on itself.
 */
export function evaluateStatusChecks(args: {
  checkRuns: readonly CheckRunLike[];
  combinedStatusState: 'success' | 'pending' | 'failure';
  combinedStatusCount: number;
  ignoreCheckNames?: readonly string[];
}): StatusChecksResult {
  const ignored = new Set(args.ignoreCheckNames ?? []);
  let consideredCount = 0;
  for (const run of args.checkRuns) {
    if (ignored.has(run.name)) {
      continue;
    }
    consideredCount += 1;
    if (run.status !== 'completed') {
      return { passed: false, reason: `check "${run.name}" is still ${run.status}` };
    }
    if (!PASSING_CONCLUSIONS.has(run.conclusion ?? '')) {
      return { passed: false, reason: `check "${run.name}" concluded ${run.conclusion ?? 'null'}` };
    }
  }
  if (args.combinedStatusCount > 0 && args.combinedStatusState !== 'success') {
    return { passed: false, reason: `commit status is ${args.combinedStatusState}` };
  }
  // Distinguish "no checks required" from "CI hasn't reported yet": with nothing to gate on, refuse
  // rather than green-light a PR whose checks simply have not started.
  if (consideredCount === 0 && args.combinedStatusCount === 0) {
    return { passed: false, reason: 'no status checks have reported yet' };
  }
  return { passed: true };
}
