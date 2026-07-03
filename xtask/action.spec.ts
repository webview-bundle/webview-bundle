import { describe, expect, it, vi } from 'vitest';
import { type Action, runActions } from './action.ts';
import type { GitHubPort, GitPort, Ports } from './ports.ts';

function makeGitHub(overrides: Partial<GitHubPort> = {}): GitHubPort {
  return {
    findReleaseByTag: vi.fn(async () => null),
    createRelease: vi.fn(async () => ({ id: 1, htmlUrl: 'https://github.test/releases/1' })),
    updateReleaseBody: vi.fn(async () => {}),
    listReleaseAssets: vi.fn(async () => []),
    deleteReleaseAsset: vi.fn(async () => {}),
    uploadReleaseAsset: vi.fn(async () => {}),
    ...overrides,
  };
}

function makeGit(overrides: Partial<GitPort> = {}): GitPort {
  return {
    createTag: vi.fn((name: string) => name),
    pushTags: vi.fn(async () => {}),
    ...overrides,
  };
}

function makePorts(overrides: Partial<Ports> = {}): Ports {
  return {
    proc: { run: vi.fn(async () => ({ exitCode: 0, output: '' })) },
    registry: { exists: vi.fn(async () => false) },
    git: null,
    github: null,
    ...overrides,
  };
}

const publish: Action = {
  type: 'publish',
  registry: 'npm',
  manifest: '@wvb/node',
  version: '0.2.0',
  cmd: 'yarn',
  args: ['npm', 'publish'],
  path: 'packages/node',
};

describe('publish action', () => {
  it('succeeds when the command succeeds', async () => {
    const ports = makePorts();
    const result = await runActions([publish], { ports, reject: false });
    expect(result.allSucceed).toBe(true);
    expect(result.items[0]).toMatchObject({ succeed: true });
  });

  it('treats a duplicate-version rejection as already published', async () => {
    const ports = makePorts({
      proc: {
        run: vi.fn(async () => ({
          exitCode: 1,
          output: 'You cannot publish over the previously published versions: 0.2.0.',
        })),
      },
    });
    const result = await runActions([publish], { ports, reject: false });
    expect(result.allSucceed).toBe(true);
    expect(result.items[0]).toMatchObject({ succeed: true, skipped: 'already published' });
  });

  it('fails on any other publish error, keeping the output', async () => {
    const ports = makePorts({
      proc: { run: vi.fn(async () => ({ exitCode: 1, output: 'E401 unauthorized' })) },
    });
    const result = await runActions([publish], { ports, reject: false });
    expect(result.allSucceed).toBe(false);
    expect(result.items[0]).toMatchObject({ succeed: false, output: 'E401 unauthorized' });
  });
});

describe('createTag / pushTags actions', () => {
  it('creates tags through the git port', async () => {
    const git = makeGit();
    const ports = makePorts({ git, github: makeGitHub() });
    const result = await runActions([{ type: 'createTag', tag: 'core/0.2.0' }], {
      ports,
      reject: false,
    });
    expect(result.allSucceed).toBe(true);
    expect(git.createTag).toHaveBeenCalledWith('core/0.2.0');
  });

  it('only logs the push when there is no github token', async () => {
    const git = makeGit();
    const ports = makePorts({ git, github: null });
    const result = await runActions(
      [{ type: 'pushTags', refspecs: ['refs/tags/core/0.2.0:refs/tags/core/0.2.0'] }],
      { ports, reject: false }
    );
    expect(result.allSucceed).toBe(true);
    expect(result.items[0]).toMatchObject({ skipped: 'no github token' });
    expect(git.pushTags).not.toHaveBeenCalled();
  });

  it('reports a push failure without throwing', async () => {
    const git = makeGit({
      pushTags: vi.fn(async () => {
        throw new Error('remote hung up');
      }),
    });
    const ports = makePorts({ git, github: makeGitHub() });
    const result = await runActions([{ type: 'pushTags', refspecs: ['refs/tags/a:refs/tags/a'] }], {
      ports,
      reject: false,
    });
    expect(result.allSucceed).toBe(false);
  });
});

describe('ensureRelease action', () => {
  const action: Action = {
    type: 'ensureRelease',
    tag: 'core/0.2.0',
    name: 'core v0.2.0',
    body: 'notes',
    targetCommitish: 'abc',
  };

  it('creates the release when missing and returns it as data', async () => {
    const github = makeGitHub();
    const ports = makePorts({ github });
    const result = await runActions([action], { ports, reject: false });
    expect(github.createRelease).toHaveBeenCalledWith({
      tag: 'core/0.2.0',
      name: 'core v0.2.0',
      body: 'notes',
      prerelease: undefined,
      targetCommitish: 'abc',
    });
    expect(result.items[0]).toMatchObject({
      succeed: true,
      data: { id: 1, htmlUrl: 'https://github.test/releases/1' },
    });
  });

  it('reuses an existing release, refreshing the body only when asked', async () => {
    const existing = { id: 7, htmlUrl: 'https://github.test/releases/7' };
    const github = makeGitHub({ findReleaseByTag: vi.fn(async () => existing) });
    const ports = makePorts({ github });

    const reused = await runActions([action], { ports, reject: false });
    expect(github.createRelease).not.toHaveBeenCalled();
    expect(github.updateReleaseBody).not.toHaveBeenCalled();
    expect(reused.items[0]).toMatchObject({ succeed: true, data: existing });

    await runActions([{ ...action, updateBody: true }], { ports, reject: false });
    expect(github.updateReleaseBody).toHaveBeenCalledWith(7, 'notes');
  });

  it('only logs when there is no github token', async () => {
    const result = await runActions([action], { ports: makePorts(), reject: false });
    expect(result.items[0]).toMatchObject({ succeed: true, skipped: 'no github token' });
  });
});

describe('uploadAssets action', () => {
  const assets = [
    { name: 'a.zip', path: '/tmp/a.zip' },
    { name: 'b.zip', path: '/tmp/b.zip' },
  ];

  it('reuses the release ensured earlier in the same run', async () => {
    const github = makeGitHub();
    const ports = makePorts({ github });
    const result = await runActions(
      [
        { type: 'ensureRelease', tag: 'core/0.2.0', name: 'core v0.2.0' },
        { type: 'uploadAssets', tag: 'core/0.2.0', assets },
      ],
      { ports, reject: false }
    );
    expect(result.allSucceed).toBe(true);
    // `ensureRelease` already resolved the release; `uploadAssets` must not look it up again.
    expect(github.findReleaseByTag).toHaveBeenCalledTimes(1);
    expect(github.uploadReleaseAsset).toHaveBeenCalledTimes(2);
  });

  it('skips uploaded assets and replaces stubs from interrupted uploads', async () => {
    const github = makeGitHub({
      findReleaseByTag: vi.fn(async () => ({ id: 7, htmlUrl: 'u' })),
      listReleaseAssets: vi.fn(async () => [
        { id: 11, name: 'a.zip', state: 'uploaded' },
        { id: 12, name: 'b.zip', state: 'open' },
      ]),
    });
    const ports = makePorts({ github });
    const result = await runActions([{ type: 'uploadAssets', tag: 'core/0.2.0', assets }], {
      ports,
      reject: false,
    });
    expect(result.allSucceed).toBe(true);
    expect(github.deleteReleaseAsset).toHaveBeenCalledWith(12);
    expect(github.uploadReleaseAsset).toHaveBeenCalledTimes(1);
    expect(result.items[0]).toMatchObject({ data: ['a.zip', 'b.zip'] });
  });

  it('fails when the release is missing', async () => {
    const github = makeGitHub();
    const ports = makePorts({ github });
    const result = await runActions([{ type: 'uploadAssets', tag: 'core/0.2.0', assets }], {
      ports,
      reject: false,
    });
    expect(result.allSucceed).toBe(false);
  });
});

describe('dry-run', () => {
  it('logs the plan without touching any port', async () => {
    const github = makeGitHub();
    const git = makeGit();
    const proc = { run: vi.fn(async () => ({ exitCode: 0, output: '' })) };
    const ports = makePorts({ github, git, proc });
    const result = await runActions(
      [
        publish,
        { type: 'createTag', tag: 'core/0.2.0' },
        { type: 'pushTags', refspecs: ['refs/tags/core/0.2.0:refs/tags/core/0.2.0'] },
        { type: 'ensureRelease', tag: 'core/0.2.0', name: 'core v0.2.0' },
        { type: 'uploadAssets', tag: 'core/0.2.0', assets: [] },
      ],
      { ports, dryRun: true }
    );
    expect(result.allSucceed).toBe(true);
    expect(proc.run).not.toHaveBeenCalled();
    expect(git.createTag).not.toHaveBeenCalled();
    expect(github.createRelease).not.toHaveBeenCalled();
  });
});
