import { isTauri } from '@tauri-apps/api/core';
import { PLATFORM_MOCK_KEY } from './platform-mock.js';
import { getWindow } from './window.js';

export type PlatformType = 'electron' | 'tauri' | 'deno' | 'android' | 'ios';

export interface ElectronWindow {
  readonly wvbElectron: {
    readonly invoke: <T = unknown>(name: string, params?: any) => Promise<T>;
  };
}

function isElectron(): boolean {
  return getWindow<ElectronWindow>()?.wvbElectron != null;
}

export interface DenoWindow {
  readonly bindings: {
    readonly wvbInvoke: <T = unknown>(name: string, params?: any) => Promise<T>;
  };
}

function isDeno(): boolean {
  return getWindow<DenoWindow>()?.bindings != null;
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

function resolveType(): PlatformType | undefined {
  const mocked = getWindow<Record<string, PlatformType | undefined>>()[PLATFORM_MOCK_KEY];
  if (mocked != null) {
    return mocked;
  }
  if (isElectron()) {
    return 'electron';
  }
  if (isTauri()) {
    return 'tauri';
  }
  if (isDeno()) {
    return 'deno';
  }
  if (isAndroid()) {
    return 'android';
  }
  if (isIos()) {
    return 'ios';
  }
  return undefined;
}

export const platform = {
  get type(): PlatformType | undefined {
    return resolveType();
  },
  get isElectron(): boolean {
    return resolveType() === 'electron';
  },
  get isTauri(): boolean {
    return resolveType() === 'tauri';
  },
  get isDeno(): boolean {
    return resolveType() === 'deno';
  },
  get isAndroid(): boolean {
    return resolveType() === 'android';
  },
  get isIos(): boolean {
    return resolveType() === 'ios';
  },
};
