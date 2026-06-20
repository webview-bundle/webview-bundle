import { isTauri } from '@tauri-apps/api/core';
import { getWindow } from './window.js';

export type PlatformType = 'electron' | 'tauri' | 'android' | 'ios';

export interface ElectronWindow {
  readonly wvbElectron: {
    readonly invoke: <T = unknown>(name: string, params?: any) => Promise<T>;
  };
}

function isElectron(): boolean {
  return getWindow<ElectronWindow>()?.wvbElectron != null;
}

export interface AndroidWindow {
  readonly wvbAndroid: {
    readonly postMessage: (message: any) => void;
  };
}

function isAndroid(): boolean {
  return getWindow<AndroidWindow>()?.wvbAndroid != null;
}

export interface IosWindow {
  readonly webkit: {
    readonly messageHandlers: {
      readonly wvbIos: {
        readonly postMessage: (message: any) => void;
      };
    };
  };
}

function isIos(): boolean {
  return getWindow<IosWindow>()?.webkit?.messageHandlers?.wvbIos != null;
}

export const platform = {
  get type(): PlatformType | undefined {
    if (isElectron()) {
      return 'electron';
    }
    if (isTauri()) {
      return 'tauri';
    }
    if (isAndroid()) {
      return 'android';
    }
    if (isIos()) {
      return 'ios';
    }
    return undefined;
  },
  get isElectron(): boolean {
    return isElectron();
  },
  get isTauri(): boolean {
    return isTauri();
  },
  get isAndroid(): boolean {
    return isAndroid();
  },
  get isIos(): boolean {
    return isIos();
  },
};
