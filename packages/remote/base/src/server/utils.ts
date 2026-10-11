import type { HTTPResponseError } from 'hono/types';

export function isHTTPResponseError(e: unknown): e is HTTPResponseError {
  return e instanceof Error && typeof (e as HTTPResponseError).getResponse === 'function';
}

/** GET/HEAD If-None-Match uses weak comparison, including after CDN compression. */
export function matchesIfNoneMatch(header: string, etag: string): boolean {
  if (header.trim() === '*') {
    return true;
  }
  const opaqueTag = etag.replace(/^W\//, '');

  // Quoted opaque tags may contain commas; do not split the header on commas.
  return [...header.matchAll(/(?:^|,)\s*(?:W\/)?("[^"]*")\s*(?=,|$)/g)].some(
    match => match[1] === opaqueTag
  );
}
