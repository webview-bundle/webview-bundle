import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  Cancellation,
  type ErrorCode,
  getWebviewBundleError,
  isWebviewBundleError,
  Remote,
  type RemoteOnDownloadData,
  RUNTIME_VERSION,
  UPDATE_PROTOCOL_VERSION,
} from '../dist/index.js';
import { caught, errorCode } from './errors.js';
import { buildBundleData, startUpdateServer, type UpdateServer } from './update-server.js';

describe('remote', () => {
  let server: UpdateServer;
  let tmpdir: string;

  beforeAll(async () => {
    server = await startUpdateServer();
  });

  afterAll(() => server.close());

  beforeEach(async () => {
    server.bundles = [
      { name: 'app', version: '1.0.0', data: buildBundleData('app', '1.0.0') },
      { name: 'docs', version: '2.0.0', data: buildBundleData('docs', '2.0.0') },
    ];
    server.metadata = {};
    server.failWith = undefined;
    server.signer = undefined;
    tmpdir = path.join(os.tmpdir(), 'webview-bundle-node-remote', randomBytes(8).toString('hex'));
    await fs.mkdir(tmpdir, { recursive: true });
  });

  afterEach(async () => {
    try {
      await fs.rm(tmpdir, { recursive: true });
    } catch {}
  });

  function makeRemote(onDownload?: (data: RemoteOnDownloadData) => void) {
    return new Remote({ baseUrl: server.baseUrl, onDownload });
  }

  it('gets the update the server serves', async () => {
    const resp = await makeRemote().getUpdate();

    expect(resp?.update.runtimeVersion).toBe(RUNTIME_VERSION);
    expect(resp?.update.createdAt).toBe(server.createdAt);
    expect(resp?.update.bundles).toEqual([
      { name: 'app', version: '1.0.0' },
      { name: 'docs', version: '2.0.0' },
    ]);
    expect(resp?.update.metadata).toEqual({});
    expect(resp?.etag).toBeTruthy();
    expect(resp?.signature).toBeUndefined();
  });

  it('announces the protocol and runtime version it speaks', async () => {
    await makeRemote().getUpdate();

    expect(server.lastRequest?.headers.get('wvb-update-protocol-version')).toBe(
      UPDATE_PROTOCOL_VERSION
    );
    expect(server.lastRequest?.headers.get('wvb-runtime-version')).toBe(String(RUNTIME_VERSION));
    expect(server.lastRequest?.headers.get('accept')).toBe('application/json');
  });

  it('returns null when the update is not modified', async () => {
    const remote = makeRemote();
    const first = await remote.getUpdate();
    expect(first?.etag).toBeTruthy();

    expect(await remote.getUpdate({ etag: first?.etag ?? undefined })).toBeNull();
    expect(server.lastRequest?.headers.get('if-none-match')).toBe(first?.etag);
  });

  it('sends the update channel and receives what it serves', async () => {
    const resp = await makeRemote().getUpdate({ channel: 'beta' });

    expect(server.lastRequest?.headers.get('wvb-update-channel')).toBe('beta');
    expect(resp?.update.metadata).toEqual({ channel: 'beta' });
  });

  it('carries the download url and integrity of every bundle', async () => {
    server.bundles = [
      {
        name: 'app',
        version: '1.0.0',
        data: buildBundleData('app', '1.0.0'),
        downloadUrl: 'https://cdn.example.com/app.wvb',
        integrity: 'sha256:AAAA',
        metadata: { channel: 'beta' },
      },
    ];

    const resp = await makeRemote().getUpdate();
    expect(resp?.update.bundles).toEqual([
      {
        name: 'app',
        version: '1.0.0',
        downloadUrl: 'https://cdn.example.com/app.wvb',
        integrity: 'sha256:AAAA',
        metadata: { channel: 'beta' },
      },
    ]);
  });

  it('downloads a bundle into the given filepath and reports progress', async () => {
    const events: RemoteOnDownloadData[] = [];
    const remote = makeRemote(data => events.push(data));
    const filepath = path.join(tmpdir, 'app.wvb');

    await remote.download(`${server.baseUrl}/bundles/app/1.0.0`, filepath);

    expect(await fs.readFile(filepath)).toEqual(server.bundles[0]?.data);
    await vi.waitFor(() => expect(events.length).toBeGreaterThan(0));
    expect(events.at(-1)?.downloadedBytes).toBe(server.bundles[0]?.data.length);
    expect(events.at(-1)?.url).toBe(`${server.baseUrl}/bundles/app/1.0.0`);
  });

  it('cancels a download', async () => {
    const cancellation = new Cancellation();
    cancellation.cancel();
    expect(cancellation.isCancelled()).toBe(true);

    const error = await makeRemote()
      .download(`${server.baseUrl}/bundles/app/1.0.0`, path.join(tmpdir, 'app.wvb'), cancellation)
      .catch(e => e);
    expect(errorCode(error)).toBe<ErrorCode>('core.cancelled');
  });

  it('reports the message of a failing remote response', async () => {
    server.failWith = { status: 503, message: 'maintenance' };

    const error = await makeRemote()
      .getUpdate()
      .catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(errorCode(error)).toBe<ErrorCode>('core.remote_http');
    expect(getWebviewBundleError(error)?.message).toContain('503');
    expect(getWebviewBundleError(error)?.message).toContain('maintenance');
    expect(getWebviewBundleError(error)?.message).not.toMatch(/^\[/);
  });

  it('rejects a download of a bundle the server does not have', async () => {
    const error = await makeRemote()
      .download(`${server.baseUrl}/bundles/app/9.9.9`, path.join(tmpdir, 'app.wvb'))
      .catch(e => e);
    expect(errorCode(error)).toBe<ErrorCode>('core.remote_http');
  });

  it('rejects an invalid base url from the constructor', () => {
    expect(errorCode(caught(() => new Remote({ baseUrl: '' })))).toBe<ErrorCode>(
      'core.invalid_remote_config'
    );
    expect(errorCode(caught(() => new Remote({ baseUrl: 'not a url' })))).toBe<ErrorCode>(
      'core.invalid_remote_config'
    );
  });

  it('sends the configured http options on every request', async () => {
    const remote = new Remote({
      baseUrl: server.baseUrl,
      http: {
        defaultHeaders: { authorization: 'Bearer tok-123', 'x-tenant': 'acme' },
        userAgent: 'wvb-test/1.0',
      },
    });
    await remote.getUpdate();

    expect(server.lastRequest?.headers.get('authorization')).toBe('Bearer tok-123');
    expect(server.lastRequest?.headers.get('x-tenant')).toBe('acme');
    expect(server.lastRequest?.headers.get('user-agent')).toBe('wvb-test/1.0');
  });

  it('rejects a body that is not an update', async () => {
    server.bundles = [];
    server.failWith = undefined;
    const remote = makeRemote();
    const resp = await remote.getUpdate();
    expect(resp?.update.bundles).toEqual([]);

    server.failWith = { status: 200, message: 'not an update' };
    const error = await remote.getUpdate().catch(e => e);
    expect(errorCode(error)).toBe<ErrorCode>('core.serde_json');
  });

  it('passes a buffer of the exact bundle bytes through to disk', async () => {
    const filepath = path.join(tmpdir, 'docs.wvb');
    await makeRemote().download(`${server.baseUrl}/bundles/docs/2.0.0`, filepath);
    expect(Buffer.compare(await fs.readFile(filepath), server.bundles[1]!.data)).toBe(0);
  });
});
