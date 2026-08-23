/** biome-ignore-all lint/correctness/useImportExtensions: allow .cjs */
import type { ErrorCode } from '../binding.cjs';

export interface WebviewBundleError {
  code: ErrorCode;
  message: string;
}

// `src/error.rs` prefixes the message it throws with `[code=<code>] `
const TAGGED_MESSAGE = /^\[code=([a-z0-9_.]+)] ([\s\S]*)$/;

/**
 * The code and message of an error thrown by this binding, or `undefined` for anything else.
 */
export function getWebviewBundleError(value: unknown): WebviewBundleError | undefined {
  if (!(value instanceof Error)) {
    return undefined;
  }
  const matched = TAGGED_MESSAGE.exec(value.message);
  if (matched == null) {
    return undefined;
  }
  const [, code = '', message = ''] = matched;
  return { code: code as ErrorCode, message };
}

/** Whether `value` is an error thrown by this binding. */
export function isWebviewBundleError(value: unknown): value is Error {
  return getWebviewBundleError(value) != null;
}
