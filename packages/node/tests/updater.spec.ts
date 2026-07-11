import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { type ServerType, serve } from '@hono/node-server';
import getPort from 'get-port';
import { Hono } from 'hono';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import {
  BundleBuilder,
  BundleSource,
  Remote,
  Updater,
  writeBundleIntoBuffer,
} from '../dist/index.js';

let port: number;
let server: ServerType;

// Current remote version per bundle, served by the mock server below.
const currentVersion: Record<string, string> = { app: '1.0.0' };

function makeBundleResponse(bundleName: string, version: string) {
  const headers = new Headers();
  headers.set('content-type', 'application/webview-bundle');
  headers.set('webview-bundle-name', bundleName);
  headers.set('webview-bundle-version', version);
  const builder = new BundleBuilder();
  builder.insertEntry('/index.html', Buffer.from(`<h1>${bundleName}@${version}</h1>`, 'utf8'));
  const bundle = builder.build();
  const buf = writeBundleIntoBuffer(bundle);
  return new Response(new Uint8Array(buf), { status: 200, headers });
}

beforeAll(async () => {
  port = await getPort();
  const app = new Hono();
  // GET /bundles/{name} -> current version
  app.get('/bundles/:name', async c => {
    const name = c.req.param('name');
    const version = currentVersion[name];
    if (version == null) {
      return c.notFound();
    }
    return makeBundleResponse(name, version);
  });
  // GET /bundles/{name}/{version} -> specific version
  app.get('/bundles/:name/:version', async c => {
    const name = c.req.param('name');
    const version = c.req.param('version');
    return makeBundleResponse(name, version);
  });
  server = serve({ fetch: app.fetch, port });
});

afterAll(() => {
  return new Promise<void>((resolve, reject) => {
    if (server == null) {
      resolve();
      return;
    }
    server.close(e => (e != null ? reject(e) : resolve()));
  });
});

describe('updater', () => {
  let tmpdir: string;
  let builtinDir: string;
  let remoteDir: string;

  beforeEach(async () => {
    tmpdir = path.join(os.tmpdir(), 'webview-bundle-node-updater', randomBytes(8).toString('hex'));
    builtinDir = path.join(tmpdir, 'builtin');
    remoteDir = path.join(tmpdir, 'remote');
    await fs.mkdir(builtinDir, { recursive: true });
    await fs.mkdir(remoteDir, { recursive: true });
  });

  afterEach(async () => {
    try {
      await fs.rm(tmpdir, { recursive: true });
    } catch {}
  });

  function setup() {
    const source = new BundleSource({ builtinDir, remoteDir });
    const remote = new Remote(`http://localhost:${port}`);
    const updater = new Updater(source, remote);
    return { source, remote, updater };
  }

  it('download stages a version and install activates it', async () => {
    const { source, updater } = setup();

    // Nothing installed yet.
    expect(await source.loadVersion('app')).toBeNull();

    const info = await updater.download('app', '1.0.0');
    expect(info.name).toBe('app');
    expect(info.version).toBe('1.0.0');

    // A download stages the bundle but must NOT activate it.
    expect(await source.loadVersion('app')).toBeNull();

    await updater.install('app', '1.0.0');

    // Now the version is the active one and served from the remote source.
    expect(await source.loadVersion('app')).toEqual({ type: 'remote', version: '1.0.0' });

    const loaded = await source.loadDescriptor('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>app@1.0.0</h1>', 'utf8'));
  });

  it('install rejects a version that was never downloaded', async () => {
    const { updater } = setup();
    await expect(updater.install('app', '9.9.9')).rejects.toThrowError();
  });

  it('serializes concurrent downloads and installs of the same bundle', async () => {
    const { source, updater } = setup();

    // Concurrent identical downloads must not corrupt the staged bundle or deadlock.
    await Promise.all([
      updater.download('app', '1.0.0'),
      updater.download('app', '1.0.0'),
      updater.download('app', '1.0.0'),
    ]);

    await updater.install('app', '1.0.0');
    expect(await source.loadVersion('app')).toEqual({ type: 'remote', version: '1.0.0' });
  });
});
