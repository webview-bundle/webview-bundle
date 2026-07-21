import { afterEach, describe, expect, it, vi } from 'vitest';
import { formatVersion, REGISTRIES, resolveVersions, toRange } from './versions.js';

describe('toRange', () => {
  it('carets a stable version', () => {
    expect(toRange('0.1.0')).toBe('^0.1.0');
    expect(toRange('1.2.3')).toBe('^1.2.3');
  });

  // A caret on a prerelease resolves upward into the stable that supersedes it.
  it('pins a prerelease exactly instead of caretting it', () => {
    expect(toRange('0.1.0-next.5d93c7f')).toBe('0.1.0-next.5d93c7f');
  });

  it('passes non-semver values through untouched', () => {
    expect(toRange('file:../wvb-cli.tgz')).toBe('file:../wvb-cli.tgz');
    expect(toRange('latest')).toBe('latest');
  });
});

describe('formatVersion', () => {
  it('carets npm and jsr packages', () => {
    expect(formatVersion('@wvb/cli', '0.1.0')).toBe('^0.1.0');
    expect(formatVersion('@wvb/deno-desktop', '1.0.0')).toBe('^1.0.0');
  });

  // Cargo reads a bare version as a caret; SPM upToNextMajor(from:) and a Maven coordinate want the
  // bare number, so these registries are left unwrapped.
  it('leaves crates, maven and spm versions bare', () => {
    expect(formatVersion('wvb-tauri', '0.1.0')).toBe('0.1.0');
    expect(formatVersion('webview-bundle-android', '0.1.0')).toBe('0.1.0');
    expect(formatVersion('webview-bundle-ios', '0.1.0')).toBe('0.1.0');
  });
});

describe('resolveVersions', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
  });

  it('resolves each package from its registry', async () => {
    const fetchMock = vi.fn(async (url: string) => {
      const body = url.includes('registry.npmjs.org')
        ? { version: '0.3.0' }
        : url.includes('crates.io')
          ? { crate: { max_stable_version: '0.2.0' } }
          : {};
      return new Response(JSON.stringify(body), { status: 200 });
    });
    vi.stubGlobal('fetch', fetchMock);

    const versions = await resolveVersions(['@wvb/cli', 'wvb-tauri']);
    expect(versions).toEqual({ '@wvb/cli': '0.3.0', 'wvb-tauri': '0.2.0' });
  });

  it('omits a package the registry does not know', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('not found', { status: 404 }))
    );
    const versions = await resolveVersions(['webview-bundle-android']);
    expect(versions['webview-bundle-android']).toBeUndefined();
  });

  it('lets an override file win and skips the network for it', async () => {
    const fetchMock = vi.fn(async () => new Response('{}', { status: 500 }));
    vi.stubGlobal('fetch', fetchMock);
    const fs = await import('node:fs/promises');
    const os = await import('node:os');
    const path = await import('node:path');
    const tmp = path.join(os.tmpdir(), `wvb-ov-${process.pid}.json`);
    await fs.writeFile(tmp, JSON.stringify({ '@wvb/cli': 'file:/tmp/wvb-cli.tgz' }));
    try {
      const versions = await resolveVersions(['@wvb/cli'], tmp);
      expect(versions['@wvb/cli']).toBe('file:/tmp/wvb-cli.tgz');
      expect(fetchMock).not.toHaveBeenCalled();
    } finally {
      await fs.rm(tmp, { force: true });
    }
  });
});

describe('REGISTRIES', () => {
  it('covers every kind with the right coordinate shape', () => {
    expect(REGISTRIES['@wvb/cli']).toEqual({ kind: 'npm', npm: '@wvb/cli' });
    expect(REGISTRIES['wvb-tauri']).toEqual({ kind: 'crates', crate: 'wvb-tauri' });
    expect(REGISTRIES['webview-bundle-android']?.kind).toBe('maven');
    expect(REGISTRIES['webview-bundle-ios']?.kind).toBe('github-tag');
  });
});
