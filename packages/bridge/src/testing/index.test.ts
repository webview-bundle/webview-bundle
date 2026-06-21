import { afterEach, expect, test } from 'vitest';
import { BridgeError } from '../error.js';
import { platform } from '../platform.js';
import { remote } from '../remote.js';
import { source } from '../source.js';
import { updater } from '../updater.js';
import { clearInvokeMocks, mockBridge, mockInvoke, mockPlatform } from './index.js';

afterEach(clearInvokeMocks);

test('infers params and result, then answers the invoke', async () => {
  mockInvoke('source.loadVersion', bundleName => {
    expect(bundleName).toBe('app');
    return { type: 'remote', version: '1.0.0' };
  });
  await expect(source.loadVersion('app')).resolves.toEqual({ type: 'remote', version: '1.0.0' });
});

test('supports async handlers', async () => {
  mockInvoke('remote.download', async bundleName => ({ name: bundleName, version: '2.0.0' }));
  await expect(remote.download('app')).resolves.toEqual({ name: 'app', version: '2.0.0' });
});

test('rejects when no handler is registered', async () => {
  await expect(updater.listRemotes()).rejects.toThrow();
});

test('normalizes a rejected invoke into a BridgeError, preserving the code', async () => {
  mockInvoke('source.loadVersion', () => {
    throw BridgeError.of('remote_not_initialized');
  });
  const error = await source.loadVersion('app').catch(e => e);
  expect(error).toBeInstanceOf(BridgeError);
  expect(error.code).toBe('remote_not_initialized');
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
