import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  crateIndexPath,
  cratesVersionExists,
  isAlreadyPublishedRejection,
  npmVersionExists,
} from './registry.ts';

function stubFetch(handler: (url: string) => Promise<Response> | Response) {
  const mock = vi.fn((input: string | URL | Request) => Promise.resolve(handler(String(input))));
  vi.stubGlobal('fetch', mock);
  return mock;
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('npmVersionExists', () => {
  it('encodes the scoped name and answers by status', async () => {
    const mock = stubFetch(() => new Response('{}', { status: 200 }));
    await expect(npmVersionExists('@wvb/node', '0.1.0-next.abc1234')).resolves.toBe(true);
    expect(String(mock.mock.calls[0]![0])).toBe(
      'https://registry.npmjs.org/@wvb%2Fnode/0.1.0-next.abc1234'
    );
  });

  it('answers false on 404 and null on other failures', async () => {
    stubFetch(() => new Response('not found', { status: 404 }));
    await expect(npmVersionExists('@wvb/node', '0.1.0')).resolves.toBe(false);

    stubFetch(() => new Response('oops', { status: 500 }));
    await expect(npmVersionExists('@wvb/node', '0.1.0')).resolves.toBe(null);

    stubFetch(() => {
      throw new Error('network down');
    });
    await expect(npmVersionExists('@wvb/node', '0.1.0')).resolves.toBe(null);
  });
});

describe('cratesVersionExists', () => {
  const indexBody = [
    JSON.stringify({ name: 'wvb', vers: '0.1.0' }),
    JSON.stringify({ name: 'wvb', vers: '0.2.0', yanked: true }),
  ].join('\n');

  it('matches versions (including yanked) from the sparse index', async () => {
    stubFetch(() => new Response(indexBody, { status: 200 }));
    await expect(cratesVersionExists('wvb', '0.1.0')).resolves.toBe(true);
    await expect(cratesVersionExists('wvb', '0.2.0')).resolves.toBe(true);
    await expect(cratesVersionExists('wvb', '0.3.0')).resolves.toBe(false);
  });

  it('answers false for a never-published crate and null on failures', async () => {
    stubFetch(() => new Response('not found', { status: 404 }));
    await expect(cratesVersionExists('wvb', '0.1.0')).resolves.toBe(false);

    stubFetch(() => new Response('oops', { status: 503 }));
    await expect(cratesVersionExists('wvb', '0.1.0')).resolves.toBe(null);
  });

  it('requests the sparse-index path for the crate name', async () => {
    const mock = stubFetch(() => new Response(indexBody, { status: 200 }));
    await cratesVersionExists('wvb-node', '0.1.0');
    expect(String(mock.mock.calls[0]![0])).toBe('https://index.crates.io/wv/b-/wvb-node');
  });
});

describe('isAlreadyPublishedRejection', () => {
  it('recognizes duplicate-version rejections from npm and crates.io', () => {
    expect(
      isAlreadyPublishedRejection(
        'npm error 403 403 Forbidden - You cannot publish over the previously published versions: 0.2.0.'
      )
    ).toBe(true);
    expect(isAlreadyPublishedRejection('npm error code EPUBLISHCONFLICT')).toBe(true);
    expect(isAlreadyPublishedRejection('error: crate version `0.2.0` is already uploaded')).toBe(
      true
    );
  });

  it('does not match other failures', () => {
    expect(isAlreadyPublishedRejection(undefined)).toBe(false);
    expect(isAlreadyPublishedRejection('')).toBe(false);
    expect(isAlreadyPublishedRejection('npm error code E401 unauthorized')).toBe(false);
    expect(isAlreadyPublishedRejection('error: failed to verify package tarball')).toBe(false);
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
