import { Buffer } from 'node:buffer';
import { type ServerType, serve } from '@hono/node-server';
import getPort from 'get-port';
import { Hono } from 'hono';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import {
  BundleBuilder,
  isWebviewBundleError,
  Remote,
  writeBundleIntoBuffer,
} from '../dist/index.js';

let port: number;
let server: ServerType;
let allowOnlyLatest = false;
let lastRequestHeaders: Headers | undefined;
let lastRequestUrl: string | undefined;

beforeAll(async () => {
  port = await getPort();
  const app = new Hono();

  function makeBundleResponse(bundleName: string, version: string) {
    const headers = new Headers();
    headers.set('content-type', 'application/webview-bundle');
    headers.set('webview-bundle-name', bundleName);
    headers.set('webview-bundle-version', version);
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>Hello World</h1>', 'utf8'));
    const bundle = builder.build();
    const buf = writeBundleIntoBuffer(bundle);
    return new Response(new Uint8Array(buf), { status: 200, headers });
  }

  // GET /bundles
  app.get('/bundles', c => {
    lastRequestHeaders = c.req.raw.headers;
    lastRequestUrl = c.req.url;
    return c.json([
      {
        name: 'bundle1',
        version: '1.0.0',
      },
    ]);
  });
  // GET /bundles/{name}
  app.get('/bundles/:name', async c => {
    const bundleName = c.req.param('name');
    if (bundleName === 'bundle1') {
      return makeBundleResponse(bundleName, '1.0.0');
    }
    return c.notFound();
  });
  // GET /bundles/{name}/{version}
  app.get('/bundles/:name/:version', async c => {
    if (allowOnlyLatest) {
      return c.json({}, { status: 403 });
    }
    const bundleName = c.req.param('name');
    const version = c.req.param('version');
    if (bundleName === 'bundle1' && version === '1.0.0') {
      return makeBundleResponse(bundleName, version);
    }
    return c.notFound();
  });
  server = serve({ fetch: app.fetch, port });
});

afterAll(() => {
  allowOnlyLatest = false;
  return new Promise<void>((resolve, reject) => {
    if (server == null) {
      return;
    }
    server.close(e => {
      if (e != null) {
        reject(e);
      } else {
        resolve();
      }
    });
  });
});

describe('remote', () => {
  it('list bundles', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    const resp = await remote.listBundles();
    expect(resp).toEqual([{ name: 'bundle1', version: '1.0.0' }]);
  });

  it('get bundle info', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    const resp = await remote.getInfo('bundle1');
    expect(resp).toEqual({ name: 'bundle1', version: '1.0.0' });
  });

  it('download bundle', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    const [info, bundle] = await remote.download('bundle1');
    expect(info).toEqual({ name: 'bundle1', version: '1.0.0' });
    expect(bundle.getData('/index.html')).toEqual(Buffer.from('<h1>Hello World</h1>', 'utf8'));
  });

  it('download bundle with specific version', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    const [info, bundle] = await remote.downloadVersion('bundle1', '1.0.0');
    expect(info).toEqual({ name: 'bundle1', version: '1.0.0' });
    expect(bundle.getData('/index.html')).toEqual(Buffer.from('<h1>Hello World</h1>', 'utf8'));

    allowOnlyLatest = true;
    await expect(remote.downloadVersion('bundle1', '1.0.0')).rejects.toThrow(
      expect.objectContaining({
        name: 'WebviewBundleError',
        code: 'core.remote_forbidden',
        message: 'remote forbidden',
      })
    );
  });

  it('bundle not found', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    await expect(remote.download('not_found')).rejects.toThrow(
      expect.objectContaining({ code: 'core.remote_bundle_not_found' })
    );
  });

  it('rejects with a coded error whose message carries no code prefix', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    const error = await remote.download('not_found').catch((e: unknown) => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect((error as Error).message).not.toMatch(/^\[/);
  });

  it('an invalid endpoint rejects from the constructor', () => {
    expect(() => new Remote('')).toThrow(
      expect.objectContaining({ code: 'core.invalid_remote_config' })
    );
  });

  it('sends the fetch channel as a query parameter', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    await remote.listBundles({ channel: 'beta' });
    expect(new URL(lastRequestUrl as string).searchParams.get('channel')).toBe('beta');
  });

  // Callers pass `{ channel }` straight through, so an undefined channel has to behave exactly
  // like passing no options at all rather than sending an empty channel.
  it('an undefined channel is identical to passing no options', async () => {
    const remote = new Remote(`http://localhost:${port}`);
    await remote.listBundles();
    const withoutOptions = lastRequestUrl;

    await remote.listBundles({ channel: undefined });
    expect(lastRequestUrl).toBe(withoutOptions);
    expect(new URL(lastRequestUrl as string).searchParams.has('channel')).toBe(false);
  });

  it('sends the configured http options on every request', async () => {
    const remote = new Remote(`http://localhost:${port}`, {
      http: {
        defaultHeaders: { authorization: 'Bearer tok-123', 'x-tenant': 'acme' },
        userAgent: 'wvb-test/1.0',
      },
    });
    await remote.listBundles();

    expect(lastRequestHeaders?.get('authorization')).toBe('Bearer tok-123');
    expect(lastRequestHeaders?.get('x-tenant')).toBe('acme');
    expect(lastRequestHeaders?.get('user-agent')).toBe('wvb-test/1.0');
  });
});
