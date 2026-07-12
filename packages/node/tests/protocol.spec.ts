import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { type ServerType, serve } from '@hono/node-server';
import getPort from 'get-port';
import { Hono } from 'hono';
import {
  afterAll,
  afterEach,
  assertType,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
} from 'vitest';
import {
  BundleBuilder,
  BundleProtocol,
  type BundleResolverOptions,
  BundleSource,
  ProxyProtocol,
} from '../dist/index.js';

describe('bundle protocol', () => {
  let tmpdir: string;
  let builtinDir: string;
  let remoteDir: string;

  beforeEach(async () => {
    tmpdir = path.join(os.tmpdir(), 'webview-bundle-node-protocol', randomBytes(8).toString('hex'));
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

  // Writes a remote bundle and activates it as the current version.
  async function makeSource() {
    const source = new BundleSource({ builtinDir, remoteDir });
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>index</h1>', 'utf8'));
    builder.insertEntry('/about.html', Buffer.from('<h1>about</h1>', 'utf8'));
    builder.insertEntry('/about/index.html', Buffer.from('<h1>about dir</h1>', 'utf8'));
    await source.writeRemoteBundle('app', '1.0.0', builder.build(), {});
    await source.updateRemoteVersion('app', '1.0.0');
    return source;
  }

  // Checked by `tsc --noEmit`; a no-op at runtime.
  it('takes bundleResolver as a discriminated union', () => {
    assertType<BundleResolverOptions>({ type: 'hostname', segment: 'stripSuffix' });
    assertType<BundleResolverOptions>({ type: 'hostname', segment: 1 });
    assertType<BundleResolverOptions>({ type: 'pathname', segmentIndex: 0 });
    // @ts-expect-error `allowWvbSuffixOnly` belongs to the `hostname` variant.
    assertType<BundleResolverOptions>({ type: 'pathname', allowWvbSuffixOnly: true });
    // @ts-expect-error `segmentIndex` belongs to the `pathname` variant.
    assertType<BundleResolverOptions>({ type: 'hostname', segmentIndex: 0 });
  });

  it('resolves the bundle from the first hostname segment by default', async () => {
    const protocol = new BundleProtocol(await makeSource());
    const resp = await protocol.handle('get', 'wvb://app.wvb/index.html');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe('<h1>index</h1>');
  });

  // What `bundleProtocol()` in @wvb/electron passes when its config sets neither resolver.
  it('falls back to the defaults when the options fields are unset', async () => {
    const protocol = new BundleProtocol(await makeSource(), {
      bundleResolver: undefined,
      pathResolver: undefined,
    });
    const resp = await protocol.handle('get', 'wvb://app.wvb/about');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe('<h1>about dir</h1>');
  });

  it('resolves the bundle from a path segment', async () => {
    // The path is resolved independently of the bundle name, so the segment naming the
    // bundle stays in the path and entries are looked up with it.
    const source = new BundleSource({ builtinDir, remoteDir });
    const builder = new BundleBuilder();
    builder.insertEntry('/app/index.html', Buffer.from('<h1>by path</h1>', 'utf8'));
    await source.writeRemoteBundle('app', '1.0.0', builder.build(), {});
    await source.updateRemoteVersion('app', '1.0.0');

    const protocol = new BundleProtocol(source, { bundleResolver: { type: 'pathname' } });
    const resp = await protocol.handle('get', 'wvb://cdn.example.com/app/index.html');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe('<h1>by path</h1>');

    // segmentIndex selects which segment names the bundle: 1 is "index.html" here.
    const second = new BundleProtocol(source, {
      bundleResolver: { type: 'pathname', segmentIndex: 1 },
    });
    await expect(second.handle('get', 'wvb://cdn.example.com/app/index.html')).rejects.toThrow(
      /bundle not found/
    );
  });

  it('resolves the bundle from the full hostname', async () => {
    const source = new BundleSource({ builtinDir, remoteDir });
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>full</h1>', 'utf8'));
    await source.writeRemoteBundle('app.wvb', '1.0.0', builder.build(), {});
    await source.updateRemoteVersion('app.wvb', '1.0.0');

    const protocol = new BundleProtocol(source, {
      bundleResolver: { type: 'hostname', segment: 'full' },
    });
    const resp = await protocol.handle('get', 'wvb://app.wvb/index.html');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe('<h1>full</h1>');
  });

  it('resolves the nth hostname segment', async () => {
    const protocol = new BundleProtocol(await makeSource(), {
      bundleResolver: { type: 'hostname', segment: 1 },
    });
    const resp = await protocol.handle('get', 'wvb://cdn.app.wvb/index.html');
    expect(resp.status).toBe(200);
  });

  it('rejects hosts without the `.wvb` suffix when allowWvbSuffixOnly is set', async () => {
    const protocol = new BundleProtocol(await makeSource(), {
      bundleResolver: { type: 'hostname', allowWvbSuffixOnly: true },
    });
    expect(await protocol.handle('get', 'wvb://app.wvb/index.html')).toMatchObject({ status: 200 });
    await expect(protocol.handle('get', 'wvb://app.example.com/index.html')).rejects.toThrow(
      /bundle not found/
    );
  });

  it.each([
    ['directoryIndex', '<h1>about dir</h1>'],
    ['htmlExtension', '<h1>about</h1>'],
  ] as const)('resolves an extensionless path with %s', async (pathResolver, body) => {
    const protocol = new BundleProtocol(await makeSource(), { pathResolver });
    const resp = await protocol.handle('get', 'wvb://app.wvb/about');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe(body);
  });

  it('serves the path as-is with the exact path resolver', async () => {
    const protocol = new BundleProtocol(await makeSource(), { pathResolver: 'exact' });
    expect(await protocol.handle('get', 'wvb://app.wvb/about.html')).toMatchObject({ status: 200 });
    // No `/` -> `/index.html` rewrite.
    expect(await protocol.handle('get', 'wvb://app.wvb/')).toMatchObject({ status: 404 });
  });
});

describe('proxy protocol', () => {
  let port: number;
  let server: ServerType;

  beforeAll(async () => {
    port = await getPort();
    const app = new Hono();
    app.get('/index.html', c => c.html('<h1>proxied</h1>'));
    app.get('/api/data', c => c.json({ foo: c.req.query('foo') }));
    app.post('/api/echo', async c => c.json({ received: await c.req.json() }));
    server = serve({ fetch: app.fetch, port });
  });

  afterAll(
    () =>
      new Promise<void>((resolve, reject) => {
        if (server == null) {
          return resolve();
        }
        server.close(e => (e != null ? reject(e) : resolve()));
      })
  );

  it('proxies by host mapping', async () => {
    const protocol = new ProxyProtocol({ 'app.wvb': `http://localhost:${port}` });
    const resp = await protocol.handle('get', 'wvb://app.wvb/index.html');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe('<h1>proxied</h1>');
  });

  it('appends the path and query of the request to the proxy target', async () => {
    const protocol = new ProxyProtocol({ 'app.wvb': `http://localhost:${port}` });
    const resp = await protocol.handle('get', 'wvb://app.wvb/api/data?foo=bar');
    expect(resp.status).toBe(200);
    expect(JSON.parse(resp.body.toString('utf8'))).toEqual({ foo: 'bar' });
  });

  it('rejects a host that is not mapped', async () => {
    const protocol = new ProxyProtocol({ 'app.wvb': `http://localhost:${port}` });
    await expect(protocol.handle('get', 'wvb://other.wvb/index.html')).rejects.toThrow(
      /cannot resolve proxy server/
    );
  });

  it('forwards the request body to the proxy target', async () => {
    const protocol = new ProxyProtocol({ 'app.wvb': `http://localhost:${port}` });
    const resp = await protocol.handle(
      'post',
      'wvb://app.wvb/api/echo',
      { 'content-type': 'application/json' },
      Buffer.from(JSON.stringify({ hello: 'world' }))
    );
    expect(resp.status).toBe(200);
    expect(JSON.parse(resp.body.toString('utf8'))).toEqual({ received: { hello: 'world' } });
  });

  it('proxies with a custom resolver', async () => {
    const seen: string[] = [];
    const protocol = new ProxyProtocol(async uri => {
      seen.push(uri);
      return new URL(uri).hostname === 'app.wvb' ? `http://localhost:${port}` : null;
    });
    const resp = await protocol.handle('get', 'wvb://app.wvb/index.html');
    expect(resp.status).toBe(200);
    expect(resp.body.toString('utf8')).toBe('<h1>proxied</h1>');
    expect(seen).toEqual(['wvb://app.wvb/index.html']);

    // `null` means "do not proxy".
    await expect(protocol.handle('get', 'wvb://other.wvb/index.html')).rejects.toThrow(
      /cannot resolve proxy server/
    );
  });

  it('surfaces an error thrown by the custom resolver', async () => {
    const protocol = new ProxyProtocol(async () => {
      throw new Error('boom');
    });
    await expect(protocol.handle('get', 'wvb://app.wvb/index.html')).rejects.toThrow();
  });
});
