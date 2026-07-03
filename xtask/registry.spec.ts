import { afterEach, describe, expect, it, vi } from 'vitest';
import { crateIndexPath, cratesRegistry, jsrRegistry, npmRegistry } from './registry.ts';
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

  it('answers false on 404 and null on other failures', async () => {
    stubFetch(() => new Response('not found', { status: 404 }));
    await expect(npmRegistry.exists('@wvb/node', '0.1.0')).resolves.toBe(false);

    stubFetch(() => new Response('oops', { status: 500 }));
    await expect(npmRegistry.exists('@wvb/node', '0.1.0')).resolves.toBe(null);

    stubFetch(() => {
      throw new Error('network down');
    });
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

  it('answers false for a never-published crate and null on failures', async () => {
    stubFetch(() => new Response('not found', { status: 404 }));
    await expect(cratesRegistry.exists('wvb', '0.1.0')).resolves.toBe(false);

    stubFetch(() => new Response('oops', { status: 503 }));
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

  it('answers false on 404 and null on other failures', async () => {
    stubFetch(() => new Response('{"code":"packageVersionNotFound"}', { status: 404 }));
    await expect(jsrRegistry.exists('@wvb/deno', '0.2.0')).resolves.toBe(false);

    stubFetch(() => new Response('oops', { status: 500 }));
    await expect(jsrRegistry.exists('@wvb/deno', '0.2.0')).resolves.toBe(null);

    stubFetch(() => {
      throw new Error('network down');
    });
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

describe('crateIndexPath', () => {
  it('follows the cargo sparse-index layout', () => {
    expect(crateIndexPath('a')).toBe('1/a');
    expect(crateIndexPath('ab')).toBe('2/ab');
    expect(crateIndexPath('abc')).toBe('3/a/abc');
    expect(crateIndexPath('wvb-node')).toBe('wv/b-/wvb-node');
    expect(crateIndexPath('Serde')).toBe('se/rd/serde');
  });
});
