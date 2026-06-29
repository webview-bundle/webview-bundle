import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { CallbackBag } from './callback.js';
import { BridgeError, unknownPlatform } from './error.js';
import { INVOKE_MOCK_KEY, type InvokeMockFn } from './invoke-mock.js';
import { type AndroidWindow, type ElectronWindow, type IosWindow, platform } from './platform.js';
import { snakeCase } from './utils.js';
import { getWindow } from './window.js';

export interface InvokeParams {
  [key: string | number]: any;
}

/**
 * Invokes a native bridge command and resolves its result.
 * Before using the bridge, make sure native supports webview-bundle.
 *
 * Throws a {@link BridgeError} on failure.
 */
export async function invoke<T = unknown>(name: string, params?: InvokeParams): Promise<T> {
  try {
    return await invokeInner<T>(name, params);
  } catch (error) {
    throw BridgeError.from(error);
  }
}

function invokeInner<T>(name: string, params?: InvokeParams): Promise<T> {
  const mock = getWindow<Record<string, InvokeMockFn | undefined>>()[INVOKE_MOCK_KEY];
  if (mock != null) {
    return mock(name, params) as Promise<T>;
  }

  switch (platform.type) {
    case 'electron':
      return getWindow<ElectronWindow>().wvbElectron.invoke<T>(name, params);
    case 'tauri':
      return tauriInvoke<T>(toTauriCommand(name), params);
    case 'android':
    case 'ios': {
      const bridge = getMobileBridge(platform.type);
      return new Promise<T>((resolve, reject) => {
        const bag = new CallbackBag();
        const success = bag.generate(result => {
          resolve(result as T);
          bag.clean();
        });
        const error = bag.generate(error => {
          reject(error);
          bag.clean();
        });
        try {
          bridge.postMessage(name, params, success, error);
        } catch (error) {
          bag.clean();
          reject(error);
        }
      });
    }
    default:
      unknownPlatform();
  }
}

interface MobileBridge {
  readonly postMessage: (name: string, params: any, success: string, error: string) => any;
}

function getMobileBridge(platform: 'android' | 'ios'): MobileBridge {
  switch (platform) {
    case 'android': {
      const w = getWindow<AndroidWindow>();
      return {
        postMessage: (name, params, success, error) =>
          w.wvbAndroid.postMessage(
            JSON.stringify({
              name,
              params,
              success,
              error,
            })
          ),
      };
    }
    case 'ios': {
      const w = getWindow<IosWindow>();
      return {
        postMessage: (name, params, success, error) =>
          w.webkit.messageHandlers.wvbIos.postMessage({
            name,
            params,
            success,
            error,
          }),
      };
    }
  }
}

function toTauriCommand(name: string): string {
  return `plugin:wvb-tauri|${snakeCase(name)}`;
}
