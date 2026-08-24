import type { PlatformType } from '../platform.js';
import { PLATFORM_MOCK_KEY } from '../platform-mock.js';
import { getWindow } from '../window.js';
import { type MockInvoke, registerMockInvoke } from './mock.js';
import type { MockInvokeCommand, MockInvokeHandler } from './typesmap.js';

export type { MockInvoke } from './mock.js';
export { clearInvokeMocks } from './mock.js';
export type { BridgeCommandMap, MockInvokeCommand, MockInvokeHandler } from './typesmap.js';

/**
 * Registers a mock handler that responds a single bridge `invoke` command.
 *
 * Returns a `MockInvoke` (a `Disposable`): declare it with `using` to clear the mock
 * automatically when the scope exits, call `.clear()` to remove it manually, or let it
 * persist until {@link clearInvokeMocks}.
 *
 * @example
 * ```ts
 * import { source } from '@wvb/bridge';
 * import { mockInvoke } from '@wvb/bridge/testing';
 * import { expect, test } from 'vitest';
 *
 * test('reads the current version', async () => {
 *   using _mock = mockInvoke('source.getVersion', bundleName => {
 *     expect(bundleName).toBe('app');
 *     return { source: 'remote', version: '1.0.0' };
 *   });
 *   await expect(source.getVersion('app')).resolves.toEqual({ source: 'remote', version: '1.0.0' });
 * });
 * ```
 */
export function mockInvoke<K extends MockInvokeCommand>(
  command: K,
  handler: MockInvokeHandler<K>
): MockInvoke {
  return registerMockInvoke(command, handler);
}

/**
 * Mock platform type.
 */
export function mockPlatform(type: PlatformType): Disposable {
  const host = getWindow<Record<string, PlatformType | undefined>>();
  const previous = host[PLATFORM_MOCK_KEY];
  host[PLATFORM_MOCK_KEY] = type;
  return {
    [Symbol.dispose]() {
      host[PLATFORM_MOCK_KEY] = previous;
    },
  };
}

export interface MockBridge extends Disposable {
  mockInvoke<K extends MockInvokeCommand>(command: K, handler: MockInvokeHandler<K>): this;
  clear(): void;
}

export interface MockBridgeOptions {
  /** Platform type to mock. */
  platform?: PlatformType;
}

class MockBridgeImpl implements MockBridge {
  private disposables: Disposable[] = [];

  constructor(options?: MockBridgeOptions) {
    if (options?.platform != null) {
      this.disposables.push(mockPlatform(options.platform));
    }
  }

  mockInvoke<K extends MockInvokeCommand>(command: K, handler: MockInvokeHandler<K>): this {
    this.disposables.push(mockInvoke(command, handler));
    return this;
  }

  clear(): void {
    for (const disposable of this.disposables.reverse()) {
      disposable[Symbol.dispose]();
    }
    this.disposables = [];
  }

  [Symbol.dispose]() {
    this.clear();
  }
}

/**
 * Create a mock bridge instance.
 *
 * @example
 * ```ts
 * import { source } from '@wvb/bridge';
 * import { mockBridge } from '@wvb/bridge/testing';
 * import { expect, test } from 'vitest';
 *
 * test('reads the active version', async () => {
 *   using bridge = mockBridge()
 *     .mockInvoke('source.getVersion', bundleName => {
 *       expect(bundleName).toBe('app');
 *       return { source: 'remote', version: '1.0.0' };
 *     });
 *
 *   await expect(source.getVersion('app')).resolves.toEqual({ source: 'remote', version: '1.0.0' });
 * });
 */
export function mockBridge(options?: MockBridgeOptions): MockBridge {
  return new MockBridgeImpl(options);
}
