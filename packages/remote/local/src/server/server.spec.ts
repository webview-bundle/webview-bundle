import { once } from 'node:events';
import fs from 'node:fs/promises';
import type { AddressInfo } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import type { BundleUpdate, Current, Update, UpdateSignature } from '@wvb/remote-base';
import { v7 as uuid } from 'uuid';
import { afterEach, beforeEach, describe, expect, it, onTestFinished, vi } from 'vitest';
import { webviewBundleServer } from './server.js';

let baseDir: string;

beforeEach(async () => {
  baseDir = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-local-provider-'));
});

afterEach(async () => {
  await fs.rm(baseDir, { recursive: true, force: true });
});

describe('webviewBundleRemote', () => {
  describe('serve/shutdown', () => {
    it('listens on the requested host and serves HTTP requests', async () => {
      const server = webviewBundleServer({ baseDir });
      const onListen = vi.fn();
      const instance = server.serve({ hostname: '127.0.0.1', port: 0, onListen });
      await using nodeServer = instance.server;
      await once(nodeServer, 'listening');
      const address = nodeServer.address() as AddressInfo;

      expect(address.address).toBe('127.0.0.1');
      expect(address.port).toBeGreaterThan(0);
      expect(onListen).toHaveBeenCalledExactlyOnceWith(address);

      const response = await fetch(`http://127.0.0.1:${address.port}/update`, {
        headers: { 'wvb-update-protocol-version': '1', 'wvb-runtime-version': '1' },
      });

      expect(response.status).toBe(204);
      expect(await response.text()).toBe('');
    });

    it('releases the listening port when shutdown completes', async () => {
      const server = webviewBundleServer({ baseDir });
      const instance = server.serve({ hostname: '127.0.0.1', port: 0 });
      const nodeServer = instance.server;
      onTestFinished(async () => {
        if (nodeServer.listening) {
          await instance.shutdown();
        }
      });
      await once(nodeServer, 'listening');
      const { port } = nodeServer.address() as AddressInfo;

      await expect(instance.shutdown()).resolves.toBeUndefined();

      expect(nodeServer.listening).toBe(false);
      expect(nodeServer.address()).toBeNull();

      const replacement = webviewBundleServer({ baseDir });
      await using replacementServer = replacement.serve({ hostname: '127.0.0.1', port }).server;
      await once(replacementServer, 'listening');

      expect(replacementServer.address()).toMatchObject({ address: '127.0.0.1', port });
    });

    it('rejects shutdown when the server is already stopped', async () => {
      const server = webviewBundleServer({ baseDir });
      const instance = server.serve({ hostname: '127.0.0.1', port: 0 });
      const nodeServer = instance.server;
      onTestFinished(async () => {
        if (nodeServer.listening) {
          await instance.shutdown();
        }
      });
      await once(nodeServer, 'listening');
      await instance.shutdown();

      await expect(instance.shutdown()).rejects.toMatchObject({
        code: 'ERR_SERVER_NOT_RUNNING',
      });
    });
  });

  it('rejects an unsupported update protocol version', async () => {
    const server = webviewBundleServer({ baseDir });
    const response = await server.request('/update');

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({
      message: 'Unsupported update protocol version: (none)',
    });
  });

  it.each([
    undefined,
    '',
    '0',
    '-1',
    '1.5',
    'invalid',
    'Infinity',
    '9007199254740992',
  ])('rejects an invalid runtime version: %s', async runtimeVersion => {
    const server = webviewBundleServer({ baseDir });
    const headers = new Headers({ 'wvb-update-protocol-version': '1' });
    if (runtimeVersion != null) {
      headers.set('wvb-runtime-version', runtimeVersion);
    }

    const response = await server.request('/update', { headers });

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({
      message: `Invalid runtime version: ${runtimeVersion}`,
    });
  });

  it('serves the current update and handles its etag', async () => {
    await writeUpdate();
    const server = webviewBundleServer({ baseDir });

    const response = await server.request('/update', {
      headers: {
        'wvb-update-protocol-version': '1',
        'wvb-runtime-version': '1',
      },
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

    const notModified = await server.request('/update', {
      headers: {
        'if-none-match': etag!,
        'wvb-update-protocol-version': '1',
        'wvb-runtime-version': '1',
      },
    });
    expect(notModified.status).toBe(304);
    expect(await notModified.text()).toBe('');
  });

  it('returns no content when an update does not exist', async () => {
    const server = webviewBundleServer({ baseDir });
    const response = await server.request('/update', {
      headers: { 'wvb-update-protocol-version': '1', 'wvb-runtime-version': '1' },
    });

    expect(response.status).toBe(204);
    expect(await response.text()).toBe('');
  });

  it('selects an update by channel', async () => {
    await writeUpdate('beta', [{ name: 'app', version: '2.0.0' }]);

    const server = webviewBundleServer({ baseDir });
    const response = await server.request('/update', {
      headers: {
        'wvb-update-channel': 'beta',
        'wvb-update-protocol-version': '1',
        'wvb-runtime-version': '1',
      },
    });

    expect(response.status).toBe(200);
    expect((await response.json()).bundles).toEqual([{ name: 'app', version: '2.0.0' }]);
  });

  it('returns the requested update signature', async () => {
    await writeUpdate(undefined, undefined, [
      { id: 'release', alg: 'ed25519', sig: 'c2lnbmF0dXJl' },
    ]);

    const server = webviewBundleServer({ baseDir });
    const response = await server.request('/update', {
      headers: {
        'wvb-expect-signature': 'key_id="release", alg="ed25519"',
        'wvb-update-protocol-version': '1',
        'wvb-runtime-version': '1',
      },
    });

    expect(response.status).toBe(200);
    expect(response.headers.get('wvb-signature')).toBe(
      'key_id="release", alg="ed25519", sig="c2lnbmF0dXJl"'
    );
  });

  it('rejects an unavailable requested signature under the optional policy', async () => {
    await writeUpdate();

    const server = webviewBundleServer({ baseDir });
    const response = await server.request('/update', {
      headers: {
        'wvb-expect-signature': 'key_id="release", alg="ed25519"',
        'wvb-update-protocol-version': '1',
        'wvb-runtime-version': '1',
      },
    });

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ message: 'Missing expect signature' });
  });

  it('requires a requested signature under the strict policy', async () => {
    await writeUpdate();

    const server = webviewBundleServer({ baseDir, signaturePolicy: 'strict' });
    const response = await server.request('/update', {
      headers: { 'wvb-update-protocol-version': '1', 'wvb-runtime-version': '1' },
    });

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({ message: 'Missing expect signature' });
  });

  it('downloads a versioned bundle', async () => {
    await writeBundle('app', '1.2.3', 'bundle-data');

    const server = webviewBundleServer({ baseDir });
    const response = await server.request('/bundles/app/1.2.3');

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
  const id = uuid();
  const directory =
    channel == null
      ? path.join(baseDir, 'updates', '1')
      : path.join(baseDir, 'channels', channel, 'updates', '1');
  await fs.mkdir(directory, { recursive: true });
  const update: Update = {
    data: {
      id,
      createdAt: '2026-08-28T00:00:00.000Z',
      runtimeVersion: 1,
      bundles,
      metadata: {
        environment: 'local',
      },
    },
    signatures,
  };
  await fs.writeFile(path.join(directory, `${id}.json`), JSON.stringify(update));

  const current: Current = {
    updateId: id,
    currentAt: new Date().toISOString(),
  };
  await fs.writeFile(path.join(directory, 'current.json'), JSON.stringify(current));
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
