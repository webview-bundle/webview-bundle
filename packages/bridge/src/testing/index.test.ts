import { afterEach, expect, test } from 'vitest';
import { BridgeError } from '../error.js';
import { platform } from '../platform.js';
import { remote } from '../remote.js';
import { source } from '../source.js';
import { updater } from '../updater.js';
import { clearInvokeMocks, mockBridge, mockInvoke, mockPlatform } from './index.js';

afterEach(clearInvokeMocks);

test('infers params and result, then answers the invoke', async () => {
  mockInvoke('source.getVersion', bundleName => {
    expect(bundleName).toBe('app');
    return { source: 'remote', version: '1.0.0' };
  });
  await expect(source.getVersion('app')).resolves.toEqual({ source: 'remote', version: '1.0.0' });
});

test('supports async handlers', async () => {
  mockInvoke('remote.getUpdate', async options => {
    expect(options).toEqual({ channel: 'beta' });
    return {
      update: {
        id: 'u1',
        createdAt: '2026-01-01T00:00:00Z',
        runtimeVersion: 1,
        bundles: [{ name: 'app', version: '2.0.0' }],
        metadata: {},
      },
      etag: '"v1"',
    };
  });
  const response = await remote.getUpdate({ channel: 'beta' });
  expect(response?.update.bundles).toEqual([{ name: 'app', version: '2.0.0' }]);
});

test('passes every parameter through, in order', async () => {
  mockInvoke('source.removeRemoteBundle', (bundleName, version, force) => {
    expect([bundleName, version, force]).toEqual(['app', '1.0.0', true]);
    return { name: bundleName, version, kind: 'removed' };
  });
  await expect(source.removeRemoteBundle('app', '1.0.0', true)).resolves.toEqual({
    name: 'app',
    version: '1.0.0',
    kind: 'removed',
  });

  mockInvoke('updater.download', (bundleUpdates, options) => {
    expect(options).toEqual({ concurrency: 1 });
    return bundleUpdates.map(({ name, version }) => ({
      name,
      version,
      result: { type: 'downloaded' as const },
    }));
  });
  await expect(
    updater.download([{ name: 'app', version: '2.0.0' }], { concurrency: 1 })
  ).resolves.toEqual([{ name: 'app', version: '2.0.0', result: { type: 'downloaded' } }]);
});

test('rejects when no handler is registered', async () => {
  await expect(updater.getUpdate()).rejects.toThrow();
});

test('normalizes a rejected invoke into a BridgeError, preserving the code', async () => {
  mockInvoke('source.getVersion', () => {
    throw BridgeError.of('remote_not_initialized');
  });
  const error = await source.getVersion('app').catch((e: unknown) => e);
  expect(error).toBeInstanceOf(BridgeError);
  expect((error as BridgeError).code).toBe('remote_not_initialized');
});

test('clearInvokeMocks resets the ambient store', async () => {
  mockInvoke('source.resolveFilepath', () => '/tmp/app');
  clearInvokeMocks();
  await expect(source.resolveFilepath('app')).rejects.toThrow();
});

test('using clears the mock when the scope exits', async () => {
  {
    using _mock = mockInvoke('source.resolveFilepath', () => '/scoped');
    await expect(source.resolveFilepath('app')).resolves.toBe('/scoped');
  }
  await expect(source.resolveFilepath('app')).rejects.toThrow();
});

test('mockPlatform overrides platform detection and restores on scope exit', () => {
  expect(platform.type).toBeUndefined();
  {
    using _platform = mockPlatform('ios');
    expect(platform.type).toBe('ios');
    expect(platform.isIos).toBe(true);
    expect(platform.isElectron).toBe(false);
  }
  expect(platform.type).toBeUndefined();
});

test('mockBridge scopes invoke mocks and platform, then cleans up', async () => {
  {
    using bridge = mockBridge({ platform: 'android' });
    bridge.mockInvoke('source.resolveFilepath', () => '/bridged');

    expect(platform.isAndroid).toBe(true);
    await expect(source.resolveFilepath('app')).resolves.toBe('/bridged');
  }
  expect(platform.type).toBeUndefined();
  await expect(source.resolveFilepath('app')).rejects.toThrow();
});
