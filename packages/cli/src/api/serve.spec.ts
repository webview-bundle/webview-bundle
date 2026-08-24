import { Buffer } from 'node:buffer';
import fs from 'node:fs/promises';
import type { AddressInfo } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import type { ServerType } from '@hono/node-server';
import { BundleBuilder, type UriPathResolver, writeBundle } from '@wvb/node';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Logger } from '../log.js';
import { type ServeInstance, serve } from './serve.js';

let root: string;
let instance: ServeInstance | undefined;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-serve-'));
});

afterEach(async () => {
  await instance?.shutdown().catch(() => {});
  instance = undefined;
  await fs.rm(root, { recursive: true, force: true });
});

interface BundleEntry {
  data: string | Buffer;
  contentType?: string;
  headers?: Record<string, string>;
}

async function writeBundleFile(rel: string, entries: Record<string, BundleEntry>) {
  const builder = new BundleBuilder();
  for (const [entryPath, entry] of Object.entries(entries)) {
    const data = typeof entry.data === 'string' ? Buffer.from(entry.data, 'utf8') : entry.data;
    builder.insertEntry(entryPath, data, entry.contentType, entry.headers);
  }
  await writeBundle(builder.build(), path.join(root, rel));
}

async function startServe(params: {
  file: string;
  silent?: boolean;
  logger?: Logger;
  colorEnabled?: boolean;
  pathResolver?: UriPathResolver;
}) {
  instance = await serve({
    hostname: '127.0.0.1',
    port: 0,
    silent: params.silent ?? true,
    cwd: root,
    ...params,
  });
  const server: ServerType = instance.server;
  if (!server.listening) {
    await new Promise<void>(resolve => {
      server.once('listening', () => resolve());
    });
  }
  const { port } = server.address() as AddressInfo;
  return `http://127.0.0.1:${port}`;
}

function createTestLogger() {
  const messages: string[] = [];
  const record = (message: string) => {
    messages.push(message);
  };
  const logger = { debug: record, info: record, warn: record, error: record } as unknown as Logger;
  return { logger, messages };
}

describe('serve', () => {
  it('serves an entry with its content type and content length', async () => {
    await writeBundleFile('app.wvb', { '/index.html': { data: '<h1>Hello</h1>' } });
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/index.html`);

    expect(res.status).toBe(200);
    expect(res.headers.get('content-type')).toBe('text/html');
    expect(res.headers.get('content-length')).toBe('14');
    expect(await res.text()).toBe('<h1>Hello</h1>');
  });

  it('serves the headers stored on the entry', async () => {
    await writeBundleFile('app.wvb', {
      '/index.html': { data: '<h1>Hello</h1>', headers: { 'cache-control': 'no-store' } },
    });
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/index.html`);

    expect(res.headers.get('cache-control')).toBe('no-store');
  });

  it('serves binary data unchanged', async () => {
    const png = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
    await writeBundleFile('app.wvb', { '/logo.png': { data: png } });
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/logo.png`);

    expect(Buffer.from(await res.arrayBuffer())).toEqual(png);
  });

  it('returns 404 for a path that is not in the bundle', async () => {
    await writeBundleFile('app.wvb', { '/index.html': { data: '<h1>Hello</h1>' } });
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/missing.js`);

    expect(res.status).toBe(404);
  });

  it('appends the ".wvb" extension to the file argument when omitted', async () => {
    await writeBundleFile('app.wvb', { '/index.html': { data: '<h1>Hello</h1>' } });
    const baseUrl = await startServe({ file: './app' });

    const res = await fetch(`${baseUrl}/index.html`);

    expect(res.status).toBe(200);
  });

  it('throws when the bundle file does not exist', async () => {
    await expect(serve({ file: './missing', port: 0, cwd: root })).rejects.toThrow(
      `File does not exist: ${path.join(root, 'missing.wvb')}`
    );
  });

  it('logs the incoming and outgoing request lines when not silent', async () => {
    await writeBundleFile('app.wvb', { '/index.html': { data: '<h1>Hello</h1>' } });
    const { logger, messages } = createTestLogger();
    const baseUrl = await startServe({
      file: './app.wvb',
      silent: false,
      logger,
      colorEnabled: false,
    });

    await fetch(`${baseUrl}/index.html`);

    expect(messages).toContain('<-- GET /index.html');
    expect(messages.some(x => x.startsWith('--> GET /index.html 200'))).toBe(true);
  });

  it('does not log requests when silent', async () => {
    await writeBundleFile('app.wvb', { '/index.html': { data: '<h1>Hello</h1>' } });
    const { logger, messages } = createTestLogger();
    const baseUrl = await startServe({ file: './app.wvb', silent: true, logger });

    await fetch(`${baseUrl}/index.html`);

    expect(messages.some(x => x.includes('GET /index.html'))).toBe(false);
  });

  it('stops serving after shutdown', async () => {
    await writeBundleFile('app.wvb', { '/index.html': { data: '<h1>Hello</h1>' } });
    const baseUrl = await startServe({ file: './app.wvb' });

    await instance!.shutdown();

    await expect(fetch(`${baseUrl}/index.html`)).rejects.toThrow();
  });
});

describe('serve with the "directory_index" path resolver', () => {
  beforeEach(async () => {
    await writeBundleFile('app.wvb', {
      '/index.html': { data: '<h1>Root</h1>' },
      '/about/index.html': { data: '<h1>About</h1>' },
      '/about.html': { data: '<h1>About flat</h1>' },
      '/app.js': { data: 'console.log("app");' },
    });
  });

  it('is the default resolver', async () => {
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/about`);

    expect(await res.text()).toBe('<h1>About</h1>');
  });

  it('resolves the root path to "/index.html"', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'directory_index' });

    const res = await fetch(`${baseUrl}/`);

    expect(await res.text()).toBe('<h1>Root</h1>');
  });

  it('resolves a trailing-slash path to the directory index', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'directory_index' });

    const res = await fetch(`${baseUrl}/about/`);

    expect(await res.text()).toBe('<h1>About</h1>');
  });

  it('resolves an extension-less path to the directory index', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'directory_index' });

    const res = await fetch(`${baseUrl}/about`);

    expect(await res.text()).toBe('<h1>About</h1>');
  });

  it('leaves a path that already has an extension untouched', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'directory_index' });

    const res = await fetch(`${baseUrl}/app.js`);

    expect(await res.text()).toBe('console.log("app");');
  });
});

describe('serve with the "html_extension" path resolver', () => {
  beforeEach(async () => {
    await writeBundleFile('app.wvb', {
      '/index.html': { data: '<h1>Root</h1>' },
      '/about/index.html': { data: '<h1>About</h1>' },
      '/about.html': { data: '<h1>About flat</h1>' },
      '/app.js': { data: 'console.log("app");' },
    });
  });

  it('resolves the root path to "/index.html"', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'html_extension' });

    const res = await fetch(`${baseUrl}/`);

    expect(await res.text()).toBe('<h1>Root</h1>');
  });

  it('appends ".html" to an extension-less path', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'html_extension' });

    const res = await fetch(`${baseUrl}/about`);

    expect(await res.text()).toBe('<h1>About flat</h1>');
  });

  it('drops the trailing slash before appending ".html"', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'html_extension' });

    const res = await fetch(`${baseUrl}/about/`);

    expect(await res.text()).toBe('<h1>About flat</h1>');
  });

  it('leaves a path that already has an extension untouched', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'html_extension' });

    const res = await fetch(`${baseUrl}/app.js`);

    expect(await res.text()).toBe('console.log("app");');
  });
});

describe('serve with the "exact" path resolver', () => {
  beforeEach(async () => {
    await writeBundleFile('app.wvb', {
      '/index.html': { data: '<h1>Root</h1>' },
      '/about/index.html': { data: '<h1>About</h1>' },
      '/about.html': { data: '<h1>About flat</h1>' },
    });
  });

  it('serves the request path verbatim', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'exact' });

    const res = await fetch(`${baseUrl}/about/index.html`);

    expect(await res.text()).toBe('<h1>About</h1>');
  });

  it('does not resolve the root path to an index', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'exact' });

    const res = await fetch(`${baseUrl}/`);

    expect(res.status).toBe(404);
  });

  it('does not resolve an extension-less path', async () => {
    const baseUrl = await startServe({ file: './app.wvb', pathResolver: 'exact' });

    const res = await fetch(`${baseUrl}/about`);

    expect(res.status).toBe(404);
  });
});

describe('serve path decoding', () => {
  it('percent-decodes the request path before looking up the entry', async () => {
    await writeBundleFile('app.wvb', { '/hello world.html': { data: '<h1>Spaced</h1>' } });
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/hello%20world.html`);

    expect(await res.text()).toBe('<h1>Spaced</h1>');
  });

  it('falls back to the raw path when it is not a valid percent-encoding', async () => {
    await writeBundleFile('app.wvb', { '/100%.html': { data: '<h1>Percent</h1>' } });
    const baseUrl = await startServe({ file: './app.wvb' });

    const res = await fetch(`${baseUrl}/100%.html`);

    expect(await res.text()).toBe('<h1>Percent</h1>');
  });
});
