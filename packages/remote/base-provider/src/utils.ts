import type { HTTPResponseError } from 'hono/types';

export function isHTTPResponseError(e: unknown): e is HTTPResponseError {
  return e instanceof Error && typeof (e as HTTPResponseError).getResponse === 'function';
}
