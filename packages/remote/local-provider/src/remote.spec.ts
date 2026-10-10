import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import type { BundleUpdate, UpdateSignature } from '@wvb/remote-local/api';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { webviewBundleRemote } from './remote.js';

let baseDir: string;

beforeEach(async () => {
  baseDir = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-local-provider-'));
});

afterEach(async () => {
  await fs.rm(baseDir, { recursive: true, force: true });
});

describe('webviewBundleRemote', () => {
  it('rejects an unsupported update protocol version', async () => {
    const response = await webviewBundleRemote({ baseDir }).request('/update');

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({
      message: 'Unsupported update protocol version: (missing)',
    });
  });

  it('serves the current update and handles its etag', async () => {
    await writeUpdate();
    const app = webviewBundleRemote({ baseDir });

    const response = await app.request('/update', {
      headers: { 'wvb-update-protocol-version': '1' },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toContain('application/json');
    expect(await response.json()).toEqual({
      id: expect.any(String),
      createdAt: '2026-08-28T00:00:00.000Z',
      runtimeVersion: 1,
      bundles: [
        {
          name: 'app',
          version: '1.2.3',
          integrity: 'sha256-abc',
          metadata: { release: 'stable' },
        },
      ],
      metadata: { environment: 'local' },
    });

    const etag = response.headers.get('etag');
    expect(etag).toMatch(/^"[a-f0-9]{64}"$/);

    const notModified = await app.request('/update', {
      headers: {
        'if-none-match': etag!,
        'wvb-update-protocol-version': '1',
      },
    });
    expect(notModified.status).toBe(304);
    expect(await notModified.text()).toBe('');
  });

  it('returns no content when an update does not exist', async () => {
    const response = await webviewBundleRemote({ baseDir }).request('/update', {
      headers: { 'wvb-update-protocol-version': '1' },
    });

    expect(response.status).toBe(204);
    expect(await response.text()).toBe('');
  });

  it('selects an update by channel', async () => {
    await writeUpdate('beta', [{ name: 'app', version: '2.0.0' }]);

    const response = await webviewBundleRemote({ baseDir }).request('/update', {
      headers: {
        'wvb-update-channel': 'beta',
        'wvb-update-protocol-version': '1',
      },
    });

    expect(response.status).toBe(200);
    expect((await response.json()).bundles).toEqual([{ name: 'app', version: '2.0.0' }]);
  });

  it('returns the requested update signature', async () => {
    await writeUpdate(undefined, undefined, [
      { id: 'release', alg: 'ed25519', sig: 'c2lnbmF0dXJl' },
    ]);

    const response = await webviewBundleRemote({ baseDir }).request('/update', {
      headers: {
        'wvb-expect-signature': 'key_id="release", alg="ed25519"',
        'wvb-update-protocol-version': '1',
      },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get('wvb-signature')).toBe(
      'key_id="release", alg="ed25519", sig="c2lnbmF0dXJl"'
    );
  });

  it('rejects an unavailable requested signature under the optional policy', async () => {
    await writeUpdate();

    const response = await webviewBundleRemote({ baseDir }).request('/update', {
      headers: {
        'wvb-expect-signature': 'key_id="release", alg="ed25519"',
        'wvb-update-protocol-version': '1',
      },
    });

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ message: 'Missing expect signature' });
  });

  it('requires a requested signature under the strict policy', async () => {
    await writeUpdate();

    const response = await webviewBundleRemote({
      baseDir,
      signaturePolicy: 'strict',
    }).request('/update', {
      headers: { 'wvb-update-protocol-version': '1' },
    });

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ message: 'Missing expect signature' });
  });

  it('downloads a versioned bundle', async () => {
    await writeBundle('app', '1.2.3', 'bundle-data');

    const response = await webviewBundleRemote({ baseDir }).request('/bundles/app/1.2.3');

    expect(response.status).toBe(200);
    expect(response.headers.get('content-type')).toBe('application/webview-bundle');
    expect(response.headers.get('content-length')).toBe('11');
    expect(await response.text()).toBe('bundle-data');
  });
});

async function writeUpdate(
  channel?: string,
  bundles: BundleUpdate[] = [
    {
      name: 'app',
      version: '1.2.3',
      integrity: 'sha256-abc',
      metadata: { release: 'stable' },
    },
  ],
  signatures: UpdateSignature[] = []
) {
  const directory =
    channel == null
      ? path.join(baseDir, 'updates')
      : path.join(baseDir, 'updates', 'channels', channel);
  await fs.mkdir(directory, { recursive: true });
  await fs.writeFile(
    path.join(directory, 'update.json'),
    JSON.stringify({
      update: {
        id: '0198f4cb-2c00-7000-8000-000000000000',
        createdAt: '2026-08-28T00:00:00.000Z',
        runtimeVersion: 1,
        bundles,
        metadata: { environment: 'local' },
      },
      signatures,
    })
  );
}

async function writeBundle(
  name: string,
  version: string,
  contents: string,
  data?: { integrity?: string; metadata?: Record<string, string> }
) {
  const directory = path.join(baseDir, 'bundles', name);
  await fs.mkdir(directory, { recursive: true });
  await fs.writeFile(path.join(directory, `${version}.wvb`), contents);
  if (data != null) {
    await fs.writeFile(path.join(directory, `${version}_data.json`), JSON.stringify(data));
  }
}
