export class WebviewBundleError extends Error {
  override readonly name = 'WebviewBundleError';
  readonly code: string;

  constructor(code: string, message: string, options?: ErrorOptions) {
    super(message, options);
    this.code = code;
  }
}

export function isWebviewBundleError(value: unknown): value is WebviewBundleError {
  return (
    value instanceof Error &&
    value.name === 'WebviewBundleError' &&
    typeof (value as { code?: unknown }).code === 'string'
  );
}

// `src/error.rs` prefixes the message it throws with `[code=<code>] `
const TAGGED_MESSAGE = /^\[code=([a-z0-9_.]+)] ([\s\S]*)$/;

/**
 * Convert a native error into a {@link WebviewBundleError}.
 */
export function toWebviewBundleError(value: unknown): unknown {
  if (isWebviewBundleError(value) || !(value instanceof Error)) {
    return value;
  }
  const matched = TAGGED_MESSAGE.exec(value.message);
  if (matched == null) {
    return new WebviewBundleError('napi', value.message, { cause: value });
  }
  const [, code = 'unknown', message = ''] = matched;
  return new WebviewBundleError(code, message, { cause: value });
}
