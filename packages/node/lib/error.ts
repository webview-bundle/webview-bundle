/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import type { ErrorCode } from '../binding.cjs';

export interface WebviewBundleError extends Error {
  code: ErrorCode;
  message: string;
}

/** Whether `value` is an error thrown by this binding. */
export function isWebviewBundleError(value: unknown): value is WebviewBundleError {
  return value instanceof Error && value.name === 'WebviewBundleError';
}
