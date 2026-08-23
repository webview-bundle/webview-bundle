import { type ErrorCode, isWebviewBundleError } from '../dist/index.js';

export function errorCode(value: unknown): ErrorCode | undefined {
  return isWebviewBundleError(value) ? value.code : undefined;
}

export function caught(fn: () => unknown): unknown {
  try {
    fn();
  } catch (error) {
    return error;
  }
  return undefined;
}
