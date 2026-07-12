import type { ErrorCode } from './error-codes.ts';

export type { ErrorCode };

export class WebviewBundleError extends Error {
  override readonly name = 'WebviewBundleError';
  readonly code: ErrorCode;

  constructor(code: ErrorCode, message: string, options?: ErrorOptions) {
    super(message, options);
    this.code = code;
  }
}

export function isWebviewBundleError(value: unknown): value is WebviewBundleError {
  return (
    value != null &&
    typeof value === 'object' &&
    'name' in value &&
    value.name === 'WebviewBundleError'
  );
}

/**
 * Rebuild the error from the `{ code, message }` payload the native layer writes into a failed
 * `WvbResult`. Codes come from `src/error.rs`; anything else degrades to `unknown`.
 */
export function errorFromNativePayload(payload: string): WebviewBundleError {
  let parsed: unknown;
  try {
    parsed = JSON.parse(payload);
  } catch {
    return new WebviewBundleError('unknown', payload || 'wvb: operation failed');
  }
  if (parsed == null || typeof parsed !== 'object') {
    return new WebviewBundleError('unknown', payload || 'wvb: operation failed');
  }
  const { code, message } = parsed as { code?: string; message?: string };
  return new WebviewBundleError(
    (code ?? 'unknown') as ErrorCode,
    typeof message === 'string' && message.length > 0 ? message : 'wvb: operation failed'
  );
}
