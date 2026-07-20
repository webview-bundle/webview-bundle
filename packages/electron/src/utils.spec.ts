import { Buffer } from 'node:buffer';
import type { ProtocolRequest, UploadData } from 'electron';
import { describe, expect, it } from 'vitest';
import { uploadDataBody } from './utils.js';

function request(method: string, uploadData?: Partial<UploadData>[]): ProtocolRequest {
  return {
    url: 'app://myapp/api/submit',
    referrer: '',
    method,
    headers: {},
    uploadData: uploadData as UploadData[] | undefined,
  };
}

describe('uploadDataBody', () => {
  it('joins the chunks electron < 25 splits the body into', () => {
    const body = uploadDataBody(
      request('POST', [{ bytes: Buffer.from('{"hello":') }, { bytes: Buffer.from('"world"}') }])
    );
    expect(Buffer.from(body!).toString('utf8')).toBe('{"hello":"world"}');
  });

  it('has no body for a request that cannot carry one', () => {
    expect(uploadDataBody(request('GET', [{ bytes: Buffer.from('ignored') }]))).toBeUndefined();
    expect(uploadDataBody(request('HEAD'))).toBeUndefined();
  });

  it('has no body when the request carries no in-memory chunk', () => {
    expect(uploadDataBody(request('POST'))).toBeUndefined();
    expect(uploadDataBody(request('POST', []))).toBeUndefined();
    // A file or blob upload has no `bytes`.
    expect(uploadDataBody(request('POST', [{ file: '/tmp/upload' }]))).toBeUndefined();
    expect(uploadDataBody(request('POST', [{ bytes: Buffer.alloc(0) }]))).toBeUndefined();
  });
});
