import { describe, expect, it } from 'vitest';
import {
  classifyConflict,
  evaluateStatusChecks,
  indexEntryStage,
  LOCKFILE,
} from './lockfile-merge.ts';

describe('classifyConflict', () => {
  it('returns "none" when nothing conflicts', () => {
    expect(classifyConflict([])).toEqual({ type: 'none' });
    expect(classifyConflict(['', '  '])).toEqual({ type: 'none' });
  });

  it('returns "lockfile-only" when yarn.lock is the sole conflict', () => {
    expect(classifyConflict([LOCKFILE])).toEqual({ type: 'lockfile-only' });
    expect(classifyConflict([' yarn.lock '])).toEqual({ type: 'lockfile-only' });
  });

  it('returns "other" when a non-lockfile path conflicts', () => {
    expect(classifyConflict(['package.json'])).toEqual({
      type: 'other',
      files: ['package.json'],
    });
    expect(classifyConflict(['yarn.lock', 'packages/cli/src/index.ts'])).toEqual({
      type: 'other',
      files: ['yarn.lock', 'packages/cli/src/index.ts'],
    });
  });
});

describe('indexEntryStage', () => {
  it('decodes the conflict stage from libgit2 index flags', () => {
    expect(indexEntryStage(0)).toBe(0); // normally staged
    expect(indexEntryStage(0x1000)).toBe(1); // ancestor
    expect(indexEntryStage(0x2000)).toBe(2); // ours
    expect(indexEntryStage(0x3000)).toBe(3); // theirs
    // other flag bits (e.g. name length / extended) must not leak into the stage
    expect(indexEntryStage(0x2000 | 0x0fff)).toBe(2);
    expect(indexEntryStage(0x8000 | 0x1000)).toBe(1);
  });
});

describe('evaluateStatusChecks', () => {
  const ok = { combinedStatusState: 'success' as const, combinedStatusCount: 0 };

  it('passes when every check is success/neutral/skipped and no failing commit status', () => {
    expect(
      evaluateStatusChecks({
        ...ok,
        checkRuns: [
          { name: 'ci', status: 'completed', conclusion: 'success' },
          { name: 'attw', status: 'completed', conclusion: 'skipped' },
          { name: 'flaky', status: 'completed', conclusion: 'neutral' },
        ],
      })
    ).toEqual({ passed: true });
  });

  it('blocks while a check is still running', () => {
    const result = evaluateStatusChecks({
      ...ok,
      checkRuns: [{ name: 'test', status: 'in_progress', conclusion: null }],
    });
    expect(result.passed).toBe(false);
  });

  it('blocks on a failing check', () => {
    const result = evaluateStatusChecks({
      ...ok,
      checkRuns: [{ name: 'test', status: 'completed', conclusion: 'failure' }],
    });
    expect(result.passed).toBe(false);
  });

  it('blocks on a failing commit status even when check-runs are green', () => {
    const result = evaluateStatusChecks({
      checkRuns: [{ name: 'ci', status: 'completed', conclusion: 'success' }],
      combinedStatusState: 'failure',
      combinedStatusCount: 1,
    });
    expect(result.passed).toBe(false);
  });

  it('ignores the named checks (e.g. the bot itself) but still gates on the rest', () => {
    expect(
      evaluateStatusChecks({
        ...ok,
        checkRuns: [
          { name: 'resolve-lockfile', status: 'in_progress', conclusion: null },
          { name: 'ci', status: 'completed', conclusion: 'success' },
        ],
        ignoreCheckNames: ['resolve-lockfile'],
      })
    ).toEqual({ passed: true });
  });

  it('does not green-light when no checks have reported yet', () => {
    expect(evaluateStatusChecks({ ...ok, checkRuns: [] }).passed).toBe(false);
    // a lone ignored check counts as nothing reported
    expect(
      evaluateStatusChecks({
        ...ok,
        checkRuns: [{ name: 'resolve-lockfile', status: 'in_progress', conclusion: null }],
        ignoreCheckNames: ['resolve-lockfile'],
      }).passed
    ).toBe(false);
  });

  it('blocks on a pending commit-status rollup', () => {
    expect(
      evaluateStatusChecks({
        checkRuns: [{ name: 'ci', status: 'completed', conclusion: 'success' }],
        combinedStatusState: 'pending',
        combinedStatusCount: 2,
      }).passed
    ).toBe(false);
  });

  it('ignores the commit-status state when there are zero statuses', () => {
    // total_count 0 means the legacy status API is unused; a green check-run alone passes.
    expect(
      evaluateStatusChecks({
        checkRuns: [{ name: 'ci', status: 'completed', conclusion: 'success' }],
        combinedStatusState: 'pending',
        combinedStatusCount: 0,
      })
    ).toEqual({ passed: true });
  });
});
