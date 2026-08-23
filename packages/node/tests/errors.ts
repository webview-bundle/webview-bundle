import { type ErrorCode, getWebviewBundleError } from '../dist/index.js';

export function errorCode(value: unknown): ErrorCode | undefined {
  return getWebviewBundleError(value)?.code;
}

export function caught(fn: () => unknown): unknown {
  try {
    fn();
  } catch (error) {
    return error;
  }
  return undefined;
}
