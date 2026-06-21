import { contextBridge, ipcRenderer } from 'electron';
import { INVOKE_CHANNEL, type InvokeResult } from '../invoke-spec.js';

/**
 * Exposes `window.wvbElectron`, the electron transport that `@wvb/bridge` expects.
 * To use bridges in renderer process, call this function in preload script.
 *
 * @example
 * ```ts
 * // preload.ts
 * import { preload } from '@wvb/electron/preload';
 *
 * preload();
 * ```
 */
export function preload(): void {
  contextBridge.exposeInMainWorld('wvbElectron', {
    invoke: async (name: string, params?: unknown) => {
      const result: InvokeResult = await ipcRenderer.invoke(INVOKE_CHANNEL, name, params);
      if (result.ok) {
        return result.value;
      }
      return Promise.reject(result.error);
    },
  });
}
