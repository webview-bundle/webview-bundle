import { expect, it, vi } from 'vitest';
import { webviewBundleWorker } from './worker.js';

it('isolates update lookups by runtime version and channel', async () => {
  const update = {
    data: {
      id: '01936b84-8390-7000-8000-000000000000',
      createdAt: '2026-01-01T00:00:00.000Z',
      runtimeVersion: 3,
      bundles: [],
    },
    signatures: [],
  };
  const get = vi
    .fn()
    .mockImplementationOnce(async () => ({
      updateId: update.data.id,
      currentAt: update.data.createdAt,
    }))
    .mockImplementationOnce(async () => update);
  const worker = webviewBundleWorker();
  const response = await worker.request(
    '/update',
    {
      headers: {
        'wvb-update-protocol-version': '1',
        'wvb-runtime-version': '3',
        'wvb-update-channel': 'beta',
      },
    },
    { KV: { get }, BUCKET: { get: vi.fn() } }
  );
  expect(response.status).toBe(200);
  expect(await response.json()).toEqual(update.data);
  expect(get.mock.calls).toEqual([
    ['channels/beta/updates/3/current.json', 'json'],
    [`channels/beta/updates/3/${update.data.id}.json`, 'json'],
  ]);
});

it('returns 204 for a runtime with no deployment', async () => {
  const get = vi.fn().mockImplementation(async () => null);
  const worker = webviewBundleWorker();
  const response = await worker.request(
    '/update',
    { headers: { 'wvb-update-protocol-version': '1', 'wvb-runtime-version': '4' } },
    { KV: { get }, BUCKET: { get: vi.fn() } }
  );
  expect(response.status).toBe(204);
  expect(get).toHaveBeenCalledExactlyOnceWith('updates/4/current.json', 'json');
});

it('does not expose storage errors as successful responses', async () => {
  const get = vi.fn().mockRejectedValue(new Error('storage unavailable'));
  const worker = webviewBundleWorker();
  const response = await worker.request(
    '/update',
    { headers: { 'wvb-update-protocol-version': '1', 'wvb-runtime-version': '1' } },
    { KV: { get }, BUCKET: { get: vi.fn() } }
  );
  expect(response.status).toBe(500);
});

it('streams the bundle stored under the uploader key', async () => {
  const get = vi.fn().mockImplementation(async () => ({
    body: new Response('bundle').body,
    size: 6,
    httpEtag: '"bundle"',
    writeHttpMetadata: vi.fn(),
  }));
  const worker = webviewBundleWorker();
  const response = await worker.request(
    '/bundles/main/v1',
    {},
    { KV: { get: vi.fn() }, BUCKET: { get } }
  );
  expect(response.status).toBe(200);
  expect(await response.text()).toBe('bundle');
  expect(response.headers.get('content-type')).toBe('application/webview-bundle');
  expect(get).toHaveBeenCalledExactlyOnceWith('bundles/main/v1.wvb');
});

it('returns 404 when a requested bundle does not exist', async () => {
  const worker = webviewBundleWorker();
  const response = await worker.request(
    '/bundles/main/missing',
    {},
    { KV: { get: vi.fn() }, BUCKET: { get: vi.fn().mockImplementation(async () => null) } }
  );
  expect(response.status).toBe(404);
});

it('honors strict signature policy', async () => {
  const update = {
    data: {
      id: '01936b84-8390-7000-8000-000000000000',
      createdAt: '2026-01-01T00:00:00.000Z',
      runtimeVersion: 1,
      bundles: [],
    },
    signatures: [],
  };
  const get = vi
    .fn()
    .mockImplementationOnce(async () => ({
      updateId: update.data.id,
      currentAt: update.data.createdAt,
    }))
    .mockImplementationOnce(async () => update);
  const response = await webviewBundleWorker({ signaturePolicy: 'strict' }).request(
    '/update',
    { headers: { 'wvb-update-protocol-version': '1', 'wvb-runtime-version': '1' } },
    { KV: { get }, BUCKET: { get: vi.fn() } }
  );
  expect(response.status).toBe(400);
});
