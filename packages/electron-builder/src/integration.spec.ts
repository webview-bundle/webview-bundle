import path from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AfterPackContext } from './config.js';

const h = vi.hoisted(() => ({
  resolveConfig: vi.fn(),
  builtin: vi.fn(),
  rm: vi.fn(async () => {}),
  cp: vi.fn(async () => {}),
}));

vi.mock('@wvb/cli', () => ({ resolveConfig: h.resolveConfig }));
vi.mock('@wvb/cli/api', () => ({ builtin: h.builtin }));
vi.mock('node:fs/promises', () => {
  const api = { rm: h.rm, cp: h.cp };
  return { ...api, default: api };
});

const { resolveResourcesPath, webViewBundleAfterPack, withWebViewBundle } = await import(
  './integration.js'
);

function darwinContext(overrides: Partial<AfterPackContext> = {}): AfterPackContext {
  return {
    appOutDir: '/out/mac',
    electronPlatformName: 'darwin',
    arch: 1,
    packager: { projectDir: '/project', appInfo: { productFilename: 'My App' } },
    ...overrides,
  };
}

beforeEach(() => {
  h.resolveConfig.mockResolvedValue({
    root: '/project',
    remote: { endpoint: 'https://cdn.example.com' },
    builtin: { target: { type: 'remote', endpoint: 'https://cdn.example.com' } },
  });
  h.builtin.mockResolvedValue({
    manifestVersion: 1,
    entries: { app: { versions: {}, currentVersion: '1.0.0' } },
  });
});

describe('resolveResourcesPath', () => {
  it('resolves the macOS .app Resources dir for darwin and mas', () => {
    const expected = path.join('/out/mac', 'My App.app', 'Contents', 'Resources');
    expect(resolveResourcesPath(darwinContext())).toBe(expected);
    expect(resolveResourcesPath(darwinContext({ electronPlatformName: 'mas' }))).toBe(expected);
  });

  it('resolves <appOutDir>/resources for windows and linux', () => {
    expect(
      resolveResourcesPath({
        appOutDir: '/out/win',
        electronPlatformName: 'win32',
        arch: 1,
        packager: { projectDir: '/project', appInfo: { productFilename: 'My App' } },
      })
    ).toBe(path.join('/out/win', 'resources'));
    expect(
      resolveResourcesPath({
        appOutDir: '/out/linux',
        electronPlatformName: 'linux',
        arch: 1,
        packager: { projectDir: '/project', appInfo: { productFilename: 'My App' } },
      })
    ).toBe(path.join('/out/linux', 'resources'));
  });
});

describe('webViewBundleAfterPack', () => {
  it('installs builtin bundles and copies them into Resources/bundles', async () => {
    await webViewBundleAfterPack()(darwinContext());

    expect(h.builtin).toHaveBeenCalledTimes(1);
    // Staging is per (platform, arch): `<root>/<outDir>/<platform>-<arch>`.
    const stageDir = path.resolve('/project', '.wvb', 'builtin', 'bundles', 'darwin-1');
    const destDir = path.join('/out/mac', 'My App.app', 'Contents', 'Resources', 'bundles');
    expect(h.rm).toHaveBeenCalledWith(destDir, { recursive: true, force: true });
    expect(h.cp).toHaveBeenCalledWith(stageDir, destDir, { recursive: true });
  });

  it('honors a custom bundlesDir', async () => {
    await webViewBundleAfterPack({ bundlesDir: 'wvb-bundles' })(darwinContext());
    const destDir = path.join('/out/mac', 'My App.app', 'Contents', 'Resources', 'wvb-bundles');
    expect(h.cp).toHaveBeenCalledWith(expect.any(String), destDir, { recursive: true });
  });

  it('defaults a remote endpoint from remote.endpoint when the target omits it', async () => {
    h.resolveConfig.mockResolvedValue({
      root: '/project',
      remote: { endpoint: 'https://cdn.example.com' },
      builtin: { target: { type: 'remote' } },
    });
    await webViewBundleAfterPack()(darwinContext());
    expect(h.builtin).toHaveBeenCalledWith(
      expect.objectContaining({
        target: { type: 'remote', endpoint: 'https://cdn.example.com' },
      })
    );
  });

  it('throws when no builtin config resolves (default)', async () => {
    h.resolveConfig.mockResolvedValue({ root: '/project', builtin: null });
    await expect(webViewBundleAfterPack()(darwinContext())).rejects.toThrow(/No "builtin" config/);
    expect(h.builtin).not.toHaveBeenCalled();
  });

  it('no-ops when no builtin config resolves and throwWhenBuiltinIsEmpty is false', async () => {
    h.resolveConfig.mockResolvedValue({ root: '/project', builtin: null });
    await expect(
      webViewBundleAfterPack({ throwWhenBuiltinIsEmpty: false })(darwinContext())
    ).resolves.toBeUndefined();
    expect(h.builtin).not.toHaveBeenCalled();
    expect(h.cp).not.toHaveBeenCalled();
  });

  it('throws when the install produces zero bundles (default)', async () => {
    h.builtin.mockResolvedValue({ manifestVersion: 1, entries: {} });
    await expect(webViewBundleAfterPack()(darwinContext())).rejects.toThrow(
      /No builtin bundles were installed/
    );
  });

  it('allows an empty install when throwWhenBuiltinIsEmpty is false', async () => {
    h.builtin.mockResolvedValue({ manifestVersion: 1, entries: {} });
    await expect(
      webViewBundleAfterPack({ throwWhenBuiltinIsEmpty: false })(darwinContext())
    ).resolves.toBeUndefined();
    expect(h.cp).toHaveBeenCalledTimes(1);
  });
});

describe('withWebViewBundle', () => {
  it('preserves the original config and injects an afterPack hook', () => {
    const config = withWebViewBundle({ appId: 'com.example.app', asar: true });
    expect(config.appId).toBe('com.example.app');
    expect(config.asar).toBe(true);
    // `withWebViewBundle` returns the input type `C` (it doesn't surface the injected hook in the
    // type), so reach for the runtime-injected `afterPack` through a cast.
    expect(typeof (config as { afterPack?: unknown }).afterPack).toBe('function');
  });

  it('composes an existing afterPack: the existing hook runs before the install', async () => {
    const calls: string[] = [];
    const existing = vi.fn(async (_context: AfterPackContext) => {
      calls.push('existing');
    });
    h.builtin.mockImplementation(async () => {
      calls.push('install');
      return { manifestVersion: 1, entries: { app: { versions: {}, currentVersion: '1.0.0' } } };
    });

    const config = withWebViewBundle({ afterPack: existing });
    await config.afterPack(darwinContext());

    expect(existing).toHaveBeenCalledTimes(1);
    expect(calls).toEqual(['existing', 'install']);
  });

  it('throws rather than silently dropping a string (module-path) afterPack', () => {
    expect(() => withWebViewBundle({ afterPack: './my-hook.cjs' })).toThrow(
      /cannot compose an existing `afterPack` of type "string"/
    );
  });

  it('works when there is no existing afterPack', async () => {
    const config = withWebViewBundle({ appId: 'com.example.app' });
    const afterPack = (config as { afterPack?: (c: AfterPackContext) => Promise<void> }).afterPack;
    expect(afterPack).toBeTypeOf('function');
    await afterPack?.(darwinContext());
    expect(h.cp).toHaveBeenCalledTimes(1);
  });
});
