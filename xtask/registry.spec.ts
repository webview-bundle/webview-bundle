import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  crateIndexPath,
  cratesRegistry,
  fetchWithRetry,
  jsrRegistry,
  npmRegistry,
} from './registry.ts';
import { Version } from './version.ts';

function stubFetch(handler: (url: string) => Promise<Response> | Response) {
  const mock = vi.fn((input: string | URL | Request) => Promise.resolve(handler(String(input))));
  vi.stubGlobal('fetch', mock);
  return mock;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('npmRegistry', () => {
  it('encodes the scoped name and answers existence by status', async () => {
    const mock = stubFetch(() => new Response('{}', { status: 200 }));
    await expect(npmRegistry.exists('@wvb/node', '0.1.0-next.abc1234')).resolves.toBe(true);
    expect(String(mock.mock.calls[0]![0])).toBe(
      'https://registry.npmjs.org/@wvb%2Fnode/0.1.0-next.abc1234'
    );
  });

  it('answers false on 404 and null on an unexpected status', async () => {
    stubFetch(() => new Response('not found', { status: 404 }));
    await expect(npmRegistry.exists('@wvb/node', '0.1.0')).resolves.toBe(false);

    // 403 is not retryable, so this stays fast; retry/backoff is covered by `fetchWithRetry`.
    stubFetch(() => new Response('forbidden', { status: 403 }));
    await expect(npmRegistry.exists('@wvb/node', '0.1.0')).resolves.toBe(null);
  });

  it('recognizes duplicate-version rejections', () => {
    expect(
      npmRegistry.isDuplicateRejection(
        'npm error 403 403 Forbidden - You cannot publish over the previously published versions: 0.2.0.'
      )
    ).toBe(true);
    expect(npmRegistry.isDuplicateRejection('npm error code EPUBLISHCONFLICT')).toBe(true);
    expect(npmRegistry.isDuplicateRejection('npm error code E401 unauthorized')).toBe(false);
  });

  it('publishes prereleases under their channel tag, stable versions staged', () => {
    const dir = 'packages/node';
    expect(
      npmRegistry.publishCommand({
        name: '@wvb/node',
        dir,
        version: Version.parse('0.2.0-next.abc1234'),
      })
    ).toEqual({
      cmd: 'yarn',
      args: ['npm', 'publish', '--access=public', '--provenance', '--tag=next'],
      path: dir,
    });
    expect(
      npmRegistry.publishCommand({ name: '@wvb/node', dir, version: Version.parse('0.2.0') })
    ).toEqual({
      cmd: 'yarn',
      args: ['npm', 'publish', '--access=public', '--provenance', '--staged'],
      path: dir,
    });
    expect(
      npmRegistry.publishCommand({
        name: '@wvb/node',
        dir,
        version: Version.parse('0.2.1'),
        distTag: 'v0.2',
      })
    ).toEqual({
      cmd: 'yarn',
      args: ['npm', 'publish', '--access=public', '--provenance', '--tag=v0.2', '--staged'],
      path: dir,
    });
  });
});

describe('cratesRegistry', () => {
  const indexBody = [
    JSON.stringify({ name: 'wvb', vers: '0.1.0' }),
    JSON.stringify({ name: 'wvb', vers: '0.2.0', yanked: true }),
  ].join('\n');

  it('matches versions (including yanked) from the sparse index', async () => {
    stubFetch(() => new Response(indexBody, { status: 200 }));
    await expect(cratesRegistry.exists('wvb', '0.1.0')).resolves.toBe(true);
    await expect(cratesRegistry.exists('wvb', '0.2.0')).resolves.toBe(true);
    await expect(cratesRegistry.exists('wvb', '0.3.0')).resolves.toBe(false);
  });

  it('answers false for a never-published crate and null on an unexpected status', async () => {
    stubFetch(() => new Response('not found', { status: 404 }));
    await expect(cratesRegistry.exists('wvb', '0.1.0')).resolves.toBe(false);

    // 403 is not retryable, so this stays fast; retry/backoff is covered by `fetchWithRetry`.
    stubFetch(() => new Response('forbidden', { status: 403 }));
    await expect(cratesRegistry.exists('wvb', '0.1.0')).resolves.toBe(null);
  });

  it('requests the sparse-index path for the crate name', async () => {
    const mock = stubFetch(() => new Response(indexBody, { status: 200 }));
    await cratesRegistry.exists('wvb-node', '0.1.0');
    expect(String(mock.mock.calls[0]![0])).toBe('https://index.crates.io/wv/b-/wvb-node');
  });

  it('recognizes duplicate-version rejections', () => {
    expect(
      cratesRegistry.isDuplicateRejection('error: crate version `0.2.0` is already uploaded')
    ).toBe(true);
    expect(cratesRegistry.isDuplicateRejection('error: failed to verify package tarball')).toBe(
      false
    );
  });

  it('publishes from the workspace root', () => {
    expect(
      cratesRegistry.publishCommand({
        name: 'wvb',
        dir: 'packages/core',
        version: Version.parse('0.2.0'),
      })
    ).toEqual({ cmd: 'cargo', args: ['publish', '--allow-dirty', '-p', 'wvb'], path: '' });
  });
});

describe('jsrRegistry', () => {
  it('builds the jsr.io version URL', () => {
    expect(jsrRegistry.url('@wvb/deno', '0.2.0')).toBe('https://jsr.io/@wvb/deno@0.2.0');
  });

  it('splits @scope/name into the JSR API path and answers by status', async () => {
    const mock = stubFetch(() => new Response('{}', { status: 200 }));
    await expect(jsrRegistry.exists('@wvb/deno', '0.2.0-next.abc1234')).resolves.toBe(true);
    expect(String(mock.mock.calls[0]![0])).toBe(
      'https://api.jsr.io/scopes/wvb/packages/deno/versions/0.2.0-next.abc1234'
    );
  });

  it('answers false on 404 and null on an unexpected status', async () => {
    stubFetch(() => new Response('{"code":"packageVersionNotFound"}', { status: 404 }));
    await expect(jsrRegistry.exists('@wvb/deno', '0.2.0')).resolves.toBe(false);

    // 403 is not retryable, so this stays fast; retry/backoff is covered by `fetchWithRetry`.
    stubFetch(() => new Response('forbidden', { status: 403 }));
    await expect(jsrRegistry.exists('@wvb/deno', '0.2.0')).resolves.toBe(null);
  });

  it('returns null (unknown) for an unscoped name', async () => {
    const mock = stubFetch(() => new Response('{}', { status: 200 }));
    await expect(jsrRegistry.exists('deno', '0.2.0')).resolves.toBe(null);
    expect(mock).not.toHaveBeenCalled();
  });

  it('never classifies a failure as a duplicate (deno publish exits 0 on duplicates)', () => {
    expect(
      jsrRegistry.isDuplicateRejection('Warning Skipping, already published @wvb/deno@0.2.0')
    ).toBe(false);
  });

  it('publishes with deno publish from the package directory, ignoring dist-tag', () => {
    const dir = 'packages/deno';
    expect(
      jsrRegistry.publishCommand({ name: '@wvb/deno', dir, version: Version.parse('0.2.0') })
    ).toEqual({ cmd: 'deno', args: ['publish', '--allow-dirty'], path: dir });
    expect(
      jsrRegistry.publishCommand({
        name: '@wvb/deno',
        dir,
        version: Version.parse('0.2.0-next.abc1234'),
        distTag: 'next',
      })
    ).toEqual({ cmd: 'deno', args: ['publish', '--allow-dirty'], path: dir });
  });
});

describe('fetchWithRetry', () => {
  // baseDelayMs: 0 skips the real backoff sleeps so these stay fast.
  it('retries a retryable status, then returns the eventual definitive response', async () => {
    let calls = 0;
    const mock = stubFetch(() => {
      calls += 1;
      return new Response('', { status: calls < 3 ? 503 : 200 });
    });
    const res = await fetchWithRetry('https://x.test', { baseDelayMs: 0 });
    expect(res.status).toBe(200);
    expect(mock).toHaveBeenCalledTimes(3);
  });

  it('retries a thrown network error, then succeeds', async () => {
    let calls = 0;
    stubFetch(() => {
      calls += 1;
      if (calls < 2) {
        throw new Error('network down');
      }
      return new Response('', { status: 200 });
    });
    const res = await fetchWithRetry('https://x.test', { baseDelayMs: 0 });
    expect(res.status).toBe(200);
    expect(calls).toBe(2);
  });

  it('returns the last response when retries are exhausted on a retryable status', async () => {
    const mock = stubFetch(() => new Response('', { status: 500 }));
    const res = await fetchWithRetry('https://x.test', { retries: 2, baseDelayMs: 0 });
    expect(res.status).toBe(500);
    expect(mock).toHaveBeenCalledTimes(3); // 1 + 2 retries
  });

  it('rethrows when retries are exhausted on a persistent network error', async () => {
    const mock = stubFetch(() => {
      throw new Error('still down');
    });
    await expect(fetchWithRetry('https://x.test', { retries: 2, baseDelayMs: 0 })).rejects.toThrow(
      'still down'
    );
    expect(mock).toHaveBeenCalledTimes(3);
  });

  it('does not retry a definitive status (404)', async () => {
    const mock = stubFetch(() => new Response('', { status: 404 }));
    const res = await fetchWithRetry('https://x.test', { baseDelayMs: 0 });
    expect(res.status).toBe(404);
    expect(mock).toHaveBeenCalledTimes(1);
  });
});

describe('crateIndexPath', () => {
  it('follows the cargo sparse-index layout', () => {
    expect(crateIndexPath('a')).toBe('1/a');
    expect(crateIndexPath('ab')).toBe('2/ab');
    expect(crateIndexPath('abc')).toBe('3/a/abc');
    expect(crateIndexPath('wvb-node')).toBe('wv/b-/wvb-node');
    expect(crateIndexPath('Serde')).toBe('se/rd/serde');
  });
});
