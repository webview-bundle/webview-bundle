import { INVOKE_MOCK_KEY, type InvokeMockFn } from '../invoke-mock.js';
import { getWindow } from '../window.js';

type AnyHandler = (...args: any[]) => unknown;

const store = new Map<string, AnyHandler>();

interface MockHost {
  [INVOKE_MOCK_KEY]?: InvokeMockFn;
}

function ensureMocked(): void {
  const w = getWindow<MockHost>();
  if (w[INVOKE_MOCK_KEY] != null) {
    return;
  }
  w[INVOKE_MOCK_KEY] = (name, params) => {
    const handler = store.get(name);
    if (handler == null) {
      return Promise.reject(
        new Error(`[@wvb/bridge/testing] no mock registered for invoke "${name}"`)
      );
    }
    const args = params == null ? [] : Object.values(params as Record<string, unknown>);
    return Promise.resolve().then(() => handler(...args));
  };
}

function toInvokeName(command: string): string {
  const [namespace = '', method = ''] = command.split('.');
  return `${namespace}${method.charAt(0).toUpperCase()}${method.slice(1)}`;
}

export interface MockInvoke extends Disposable {
  clear(): void;
}

export function registerMockInvoke(
  command: string,
  handler: (...args: any[]) => unknown
): MockInvoke {
  ensureMocked();
  const name = toInvokeName(command);
  store.set(name, handler);
  const clear = (): void => {
    if (store.get(name) === handler) {
      store.delete(name);
    }
  };
  return { clear, [Symbol.dispose]: clear };
}

export function clearInvokeMocks(): void {
  store.clear();
}
