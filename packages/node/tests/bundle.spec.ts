import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BundleBuilder, readBundle, writeBundle } from '../index.js';

const DEFAULT_VERSION = 'v1';

const INDEX_HTML = '!<DOCTYPE html>';
const INDEX_HTML_BUF = Buffer.from(INDEX_HTML, 'utf8');

const INDEX_JS = 'console.log("hello world");';
const INDEX_JS_BUF = Buffer.from(INDEX_JS, 'utf8');

describe('builder', () => {
  it('build bundle', () => {
    const builder = new BundleBuilder('v1');
    expect(builder.version).toEqual('v1');
    expect(builder.entryPaths()).toEqual([]);
    expect(builder.insertEntry('/index.js', INDEX_JS_BUF)).toBe(false);
    expect(builder.insertEntry('/index.html', INDEX_HTML_BUF)).toBe(false);
    expect(builder.insertEntry('/index.html', INDEX_HTML_BUF)).toBe(true);
    expect(builder.entryPaths()).toHaveLength(2);
    expect(builder.containsEntry('/index.js')).toBe(true);
    expect(builder.containsEntry('/index.html')).toBe(true);
    expect(() => builder.build()).not.toThrowError();
  });

  it('build bundle with options', () => {
    const builder = new BundleBuilder('v1');
    builder.insertEntry('/index.js', INDEX_JS_BUF);
    expect(() =>
      builder.build({
        header: {
          checksumSeed: 1,
        },
        index: {
          checksumSeed: 2,
        },
        dataChecksumSeed: 3,
      })
    ).not.toThrowError();
  });
});

describe('bundle', () => {
  it('version', () => {
    const builder = new BundleBuilder('v1');
    const bundle = builder.build();
    expect(bundle.descriptor().header().version()).toEqual('v1');
  });

  it('default version', () => {
    const builder = new BundleBuilder();
    const bundle = builder.build();
    expect(bundle.descriptor().header().version()).toEqual(DEFAULT_VERSION);
  });

  it('get data', () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.js', INDEX_JS_BUF);
    builder.insertEntry('/index.html', INDEX_HTML_BUF);
    const bundle = builder.build();
    expect(bundle.descriptor().index().containsPath('/index.js')).toBe(true);
    expect(bundle.descriptor().index().containsPath('/index.html')).toBe(true);
    expect(bundle.descriptor().index().containsPath('/not_exists')).toBe(false);
    expect(bundle.getData('/index.js')).toEqual(Buffer.from(INDEX_JS_BUF));
    expect(bundle.getData('/index.html')).toEqual(Buffer.from(INDEX_HTML_BUF));
    expect(bundle.getData('/not_exists')).toBeNull();
  });

  it('get headers', () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.js', INDEX_JS_BUF, 'text/javascript', {
      foo: 'bar',
    });
    const bundle = builder.build();
    const entry = bundle.descriptor().index().getEntry('/index.js');
    expect(entry?.contentType).toEqual('text/javascript');
    expect(entry?.contentLength).toEqual(27);
    expect(entry?.headers).toEqual({
      foo: 'bar',
    });
  });
});

describe('read/write', () => {
  let tmpdir: string;

  beforeEach(async () => {
    tmpdir = path.join(os.tmpdir(), 'webview-bundle-node-binding', randomBytes(8).toString('hex'));
    await fs.mkdir(tmpdir, { recursive: true });
  });

  afterEach(async () => {
    try {
      await fs.rm(tmpdir, { recursive: true });
    } catch {}
  });

  it('write bundle and read', async () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.js', INDEX_JS_BUF);
    builder.insertEntry('/index.html', INDEX_HTML_BUF);
    const bundle = builder.build();

    await writeBundle(bundle, path.join(tmpdir, 'bundle.wvb'));
    const loadedBundle = await readBundle(path.join(tmpdir, 'bundle.wvb'));
    expect(loadedBundle.descriptor().header().version()).toEqual('v1');
    const index = loadedBundle.descriptor().index();
    expect(Object.keys(index.entries())).toHaveLength(2);
    expect(index.containsPath('/index.js')).toBe(true);
    expect(index.containsPath('/index.html')).toBe(true);
  });
});
