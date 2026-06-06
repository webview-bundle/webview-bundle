import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BundleBuilder, BundleSource } from '../index.js';

function buildBundle(html: string) {
  const builder = new BundleBuilder();
  builder.insertEntry('/index.html', Buffer.from(html, 'utf8'));
  builder.insertEntry('/app.js', Buffer.from('console.log("app");', 'utf8'));
  return builder.build();
}

describe('source', () => {
  let tmpdir: string;
  let builtinDir: string;
  let remoteDir: string;

  beforeEach(async () => {
    tmpdir = path.join(os.tmpdir(), 'webview-bundle-node-source', randomBytes(8).toString('hex'));
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

  function makeSource() {
    return new BundleSource({ builtinDir, remoteDir });
  }

  // Writes a remote bundle and activates it as the current version.
  async function install(source: BundleSource, version: string, html: string) {
    await source.writeRemoteBundle('app', version, buildBundle(html), {});
    await source.updateRemoteVersion('app', version);
  }

  it('loadDescriptor exposes metadata and reads data lazily', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    const loaded = await source.loadDescriptor('app');
    const index = loaded.descriptor().index();
    expect(index.containsPath('/index.html')).toBe(true);
    expect(index.containsPath('/app.js')).toBe(true);
    expect(index.containsPath('/missing')).toBe(false);

    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));
    expect(await loaded.getData('/missing')).toBeNull();
    expect(typeof (await loaded.getDataChecksum('/index.html'))).toBe('number');
    expect(await loaded.getDataChecksum('/missing')).toBeNull();
  });

  it('caches descriptors and unloadDescriptor evicts the cache entry', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    await source.loadDescriptor('app');
    expect(source.unloadDescriptor('app')).toBe(true);
    // Nothing left to evict on the second call.
    expect(source.unloadDescriptor('app')).toBe(false);
  });

  it('a previously-returned descriptor pins its version across an activation swap', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    const v1 = await source.loadDescriptor('app');

    // Stage and activate a newer version.
    await install(source, '1.1.0', '<h1>v2</h1>');

    // The handle obtained before the swap still reads v1's bytes: 1.0.0 is retained
    // as the previous version, so its file stays on disk and the descriptor's
    // filepath fingerprint keeps pointing at it.
    expect(await v1.getData('/index.html')).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));

    // A fresh load resolves to the new active version.
    const v2 = await source.loadDescriptor('app');
    expect(await v2.getData('/index.html')).toEqual(Buffer.from('<h1>v2</h1>', 'utf8'));
  });

  it('handles many concurrent loads and reads without hanging', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    const results = await Promise.all(
      Array.from({ length: 50 }, async () => {
        const loaded = await source.loadDescriptor('app');
        return loaded.getData('/index.html');
      })
    );
    for (const data of results) {
      expect(data).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));
    }
  });

  it('tracks retained versions and prunes the rest', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    expect(await source.remoteRetainedVersions('app')).toEqual(['1.0.0']);

    await install(source, '1.1.0', '<h1>v2</h1>');
    expect((await source.remoteRetainedVersions('app')).sort()).toEqual(['1.0.0', '1.1.0']);

    await install(source, '1.2.0', '<h1>v3</h1>');
    // current = 1.2.0, previous = 1.1.0; 1.0.0 is now prunable.
    expect((await source.remoteRetainedVersions('app')).sort()).toEqual(['1.1.0', '1.2.0']);

    expect(await source.pruneRemoteBundles('app')).toEqual(['1.0.0']);
    // Idempotent once nothing else is removable.
    expect(await source.pruneRemoteBundles('app')).toEqual([]);
  });

  it('removeRemoteBundle deletes a staged version but protects the current one', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    // Stage 1.1.0 without activating it.
    await source.writeRemoteBundle('app', '1.1.0', buildBundle('<h1>v2</h1>'), {});

    expect(await source.removeRemoteBundle('app', '1.1.0')).toBe(true);
    expect(await source.removeRemoteBundle('app', '1.1.0')).toBe(false);

    // The active version cannot be removed.
    await expect(source.removeRemoteBundle('app', '1.0.0')).rejects.toThrowError();
  });
});
