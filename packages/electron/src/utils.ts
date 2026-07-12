import { Buffer } from 'node:buffer';
import type { ProtocolRequest } from 'electron';

export function makeError(e: unknown): Error {
  if (e instanceof Error) {
    return e;
  }
  if (typeof e === 'object' && e != null) {
    if ('stack' in e || 'name' in e) {
      return e as Error;
    }
    if (typeof (e as any).message === 'string') {
      return new Error((e as any).message);
    }
  }
  return new Error(String(e));
}

/**
 * The body of an electron < 25 request, which arrives as `uploadData` chunks. Only in-memory chunks
 * are carried: a file or blob upload has no `bytes`.
 */
export function uploadDataBody(req: ProtocolRequest): Uint8Array<ArrayBuffer> | undefined {
  const method = req.method.toUpperCase();
  // A GET/HEAD `Request` may not carry a body at all.
  if (method === 'GET' || method === 'HEAD') {
    return undefined;
  }
  const chunks = (req.uploadData ?? [])
    .map(data => data.bytes)
    .filter(bytes => bytes != null && bytes.byteLength > 0);
  return chunks.length > 0 ? new Uint8Array(Buffer.concat(chunks)) : undefined;
}
