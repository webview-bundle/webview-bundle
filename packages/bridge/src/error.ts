/**
 * Bridge error data that respond from native.
 */
export interface BridgeErrorData {
  code?: string;
  message: string;
}

/**
 * Error thrown by `@wvb/bridge` when an `invoke()` command rejects, regardless of
 * platform.
 */
export class BridgeError extends Error {
  override readonly name = 'BridgeError';
  readonly code?: string;

  static of(code: string, message = ''): BridgeError {
    return new BridgeError({ code, message });
  }

  static from(value: unknown): BridgeError {
    if (value instanceof BridgeError) {
      return value;
    }
    if (isBridgeErrorData(value)) {
      return new BridgeError(value);
    }
    if (value instanceof Error) {
      return new BridgeError({ message: value.message });
    }
    if (typeof value === 'string') {
      return new BridgeError({ message: value });
    }
    return new BridgeError({ message: 'unknown bridge error' });
  }

  constructor(data: BridgeErrorData) {
    super(data.message);
    this.code = data.code;
  }
}

export function isBridgeError(e: unknown): e is BridgeError {
  return e != null && typeof e === 'object' && 'name' in e && e.name === 'BridgeError';
}

export function isBridgeErrorData(value: unknown): value is BridgeErrorData {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const data = value as { code?: unknown; message?: unknown };
  return (
    typeof data.message === 'string' && (data.code === undefined || typeof data.code === 'string')
  );
}

export function unknownPlatform(): never {
  throw new Error('Unknown platform. Make sure native webview supports webview-bundle.');
}
