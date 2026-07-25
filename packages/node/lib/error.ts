/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import type { WebviewBundleErrorCode } from '../binding.cjs';

export interface WebviewBundleError extends Error {
  name: 'WebviewBundleError';
  code: WebviewBundleErrorCode;
}

export function isWebviewBundleError(value: unknown): value is WebviewBundleError {
  return (
    value instanceof Error &&
    value.name === 'WebviewBundleError' &&
    typeof (value as { code?: unknown }).code === 'string'
  );
}
