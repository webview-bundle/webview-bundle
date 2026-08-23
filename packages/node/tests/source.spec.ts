import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  BundleBuilder,
  type ErrorCode,
  isWebviewBundleError,
  type ManifestVersionData,
  Source,
  type SourceOptions,
  writeBundleIntoBuffer,
} from '../dist/index.js';
import { caught, errorCode } from './errors.js';

function buildBundleData(html: string) {
  const builder = new BundleBuilder();
  builder.insertEntry('/index.html', Buffer.from(html, 'utf8'));
  builder.insertEntry('/app.js', Buffer.from('console.log("app");', 'utf8'));
  return Buffer.from(writeBundleIntoBuffer(builder.build()));
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

  function makeSource(options?: SourceOptions) {
    return new Source({ builtinDir, remoteDir, options });
  }

  async function stage(source: Source, version: string, html: string, data?: ManifestVersionData) {
    const filepath = source.getRemoteBundleFilepath('app', version);
    await fs.mkdir(path.dirname(filepath), { recursive: true });
    await fs.writeFile(filepath, buildBundleData(html));
    return source.stageRemoteBundle('app', { version, data });
  }

  async function install(source: Source, version: string, html: string) {
    await stage(source, version, html);
    await source.updateRemoteVersion('app', version);
  }

  async function writeBuiltin(version: string, html: string) {
    await fs.mkdir(path.join(builtinDir, 'app'), { recursive: true });
    await fs.writeFile(path.join(builtinDir, 'app', `${version}.wvb`), buildBundleData(html));
    await fs.writeFile(
      path.join(builtinDir, 'manifest.json'),
      JSON.stringify({
        manifestVersion: 1,
        bundles: { app: { versions: { [version]: {} }, currentVersion: version } },
      })
    );
  }

  it('load exposes metadata and reads data lazily', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    const loaded = await source.load('app');
    const index = loaded.descriptor().index();
    expect(index.containsPath('/index.html')).toBe(true);
    expect(index.containsPath('/app.js')).toBe(true);
    expect(index.containsPath('/missing')).toBe(false);

    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));
    expect(await loaded.getData('/missing')).toBeNull();
    expect(typeof (await loaded.getDataChecksum('/index.html'))).toBe('number');
    expect(await loaded.getDataChecksum('/missing')).toBeNull();
  });

  it('caches descriptors and unload evicts the cache entry', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    await source.load('app');
    expect(source.unload('app')).toBe(true);
    // Nothing left to evict on the second call.
    expect(source.unload('app')).toBe(false);
  });

  it('a previously-returned descriptor pins its version across an activation swap', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    const v1 = await source.load('app');

    await install(source, '1.1.0', '<h1>v2</h1>');

    // The handle obtained before the swap still reads v1's bytes: 1.0.0 is retained
    // as the previous version, so its file stays on disk and the descriptor's
    // filepath fingerprint keeps pointing at it.
    expect(await v1.getData('/index.html')).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));

    const v2 = await source.load('app');
    expect(await v2.getData('/index.html')).toEqual(Buffer.from('<h1>v2</h1>', 'utf8'));
  });

  it('resolves the remote version first and falls back to builtin', async () => {
    await writeBuiltin('0.9.0', '<h1>builtin</h1>');
    const source = makeSource();

    expect(await source.getVersion('app')).toEqual({ source: 'builtin', version: '0.9.0' });
    expect((await source.load('app')).descriptor().index().containsPath('/index.html')).toBe(true);

    await install(source, '1.0.0', '<h1>remote</h1>');
    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '1.0.0' });
    expect(await (await source.load('app')).getData('/index.html')).toEqual(
      Buffer.from('<h1>remote</h1>', 'utf8')
    );
    expect(await source.getVersion('missing')).toBeNull();
  });

  it('lists bundles of both sources with their status', async () => {
    await writeBuiltin('0.9.0', '<h1>builtin</h1>');
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await install(source, '1.1.0', '<h1>v2</h1>');
    await stage(source, '1.2.0', '<h1>v3</h1>');

    expect(await source.listBuiltinBundles()).toEqual([
      {
        source: 'builtin',
        item: { name: 'app', version: '0.9.0', status: 'current', data: {} },
      },
    ]);

    const remote = (await source.listRemoteBundles())
      .map(x => ({ version: x.item.version, status: x.item.status }))
      .sort((a, b) => a.version.localeCompare(b.version));
    expect(remote).toEqual([
      { version: '1.0.0', status: 'previous' },
      { version: '1.1.0', status: 'current' },
      { version: '1.2.0', status: 'staged' },
    ]);
    expect(await source.listBundles()).toHaveLength(4);
  });

  it('tracks staged, current and previous versions', async () => {
    const source = makeSource();

    expect(await stage(source, '1.0.0', '<h1>v1</h1>')).toEqual({
      name: 'app',
      version: '1.0.0',
      kind: 'staged',
    });
    expect(await source.getRemoteStagedVersion('app')).toBe('1.0.0');
    expect(await source.getVersion('app')).toBeNull();

    expect(await source.updateRemoteVersion('app', '1.0.0')).toEqual({
      name: 'app',
      version: '1.0.0',
      kind: 'settled',
    });
    expect(await source.getRemoteStagedVersion('app')).toBeNull();

    await install(source, '1.1.0', '<h1>v2</h1>');
    expect(await source.getRemotePreviousVersion('app')).toBe('1.0.0');

    expect(await source.updateRemoteVersion('app', '9.9.9')).toEqual({
      name: 'app',
      version: '9.9.9',
      kind: 'version_not_exists',
    });
    expect(await source.updateRemoteVersion('missing', '1.0.0')).toEqual({
      name: 'missing',
      version: '1.0.0',
      kind: 'not_exists',
    });
  });

  it('stages and activates several bundles at once', async () => {
    const source = makeSource();
    for (const name of ['a', 'b']) {
      const filepath = source.getRemoteBundleFilepath(name, '1.0.0');
      await fs.mkdir(path.dirname(filepath), { recursive: true });
      await fs.writeFile(filepath, buildBundleData(`<h1>${name}</h1>`));
    }

    const staged = await source.stageRemoteBundles({
      a: { version: '1.0.0' },
      b: { version: '1.0.0', data: { integrity: 'sha256:x' } },
    });
    expect(staged.map(x => x.kind)).toEqual(['staged', 'staged']);
    expect(await source.getRemoteVersionData('b', '1.0.0')).toEqual({ integrity: 'sha256:x' });

    const activated = await source.updateRemoteVersions({ a: '1.0.0', b: '1.0.0' });
    expect(activated.map(x => x.kind)).toEqual(['settled', 'settled']);
    expect(await source.getVersion('a')).toEqual({ source: 'remote', version: '1.0.0' });
    expect(await source.getVersion('b')).toEqual({ source: 'remote', version: '1.0.0' });
  });

  it('keeps the version data recorded when staging', async () => {
    const source = makeSource();
    await stage(source, '1.0.0', '<h1>v1</h1>', {
      integrity: 'sha256:AAAA',
      metadata: { channel: 'beta' },
    });

    expect(await source.getRemoteVersionData('app', '1.0.0')).toEqual({
      integrity: 'sha256:AAAA',
      metadata: { channel: 'beta' },
    });
    expect(await source.getRemoteVersionData('app', '9.9.9')).toBeNull();
  });

  it('reports a staged version that is already in use', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    expect(await stage(source, '1.0.0', '<h1>v1</h1>')).toEqual({
      name: 'app',
      version: '1.0.0',
      kind: 'in_use',
    });
  });

  it('resolves the filepath of the version being served', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    expect(await source.resolveFilepath('app')).toBe(
      source.getRemoteBundleFilepath('app', '1.0.0')
    );
    expect(source.getBuiltinBundleFilepath('app', '1.0.0')).toBe(
      path.join(builtinDir, 'app', '1.0.0.wvb')
    );
    expect(errorCode(caught(() => source.getRemoteBundleFilepath('..', '1.0.0')))).toBe<ErrorCode>(
      'core.invalid_filepath'
    );
  });

  it('fetches a bundle and its descriptor', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await stage(source, '1.1.0', '<h1>v2</h1>');

    expect((await source.fetchBundle('app')).getData('/index.html')).toEqual(
      Buffer.from('<h1>v1</h1>', 'utf8')
    );
    expect((await source.fetchRemoteBundle('app', '1.1.0')).getData('/index.html')).toEqual(
      Buffer.from('<h1>v2</h1>', 'utf8')
    );
    expect((await source.fetchDescriptor('app')).index().containsPath('/app.js')).toBe(true);
  });

  it('fetches a builtin bundle by version', async () => {
    await writeBuiltin('0.9.0', '<h1>builtin</h1>');
    const source = makeSource();
    expect((await source.fetchBuiltinBundle('app', '0.9.0')).getData('/index.html')).toEqual(
      Buffer.from('<h1>builtin</h1>', 'utf8')
    );
  });

  // Flips the entry's stored 4-byte checksum on disk, leaving its compressed bytes intact:
  // a read that verifies the checksum fails, a read that does not still returns the
  // original bytes.
  async function corruptChecksum(source: Source, entryPath: string) {
    const filepath = await source.resolveFilepath('app');
    const descriptor = await source.fetchDescriptor('app');
    const entry = descriptor.index().getEntry(entryPath);
    if (entry == null) {
      throw new Error(`no such entry: ${entryPath}`);
    }
    const dataOffset = Number(descriptor.header().indexEndOffset());
    const raw = await fs.readFile(filepath);
    const offset = dataOffset + entry.offset + entry.len;
    raw[offset] = raw[offset]! ^ 0xff;
    await fs.writeFile(filepath, raw);
  }

  // Flips a byte of the header's stored 4-byte checksum (fixed offset 13), leaving the header
  // fields intact: a load that verifies the header checksum fails, a load that does not still
  // parses the bundle.
  async function corruptHeaderChecksum(source: Source) {
    const filepath = await source.resolveFilepath('app');
    const raw = await fs.readFile(filepath);
    raw[13] = raw[13]! ^ 0xff;
    await fs.writeFile(filepath, raw);
  }

  it('verifies entry checksums by default', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await corruptChecksum(source, '/index.html');

    const loaded = await source.load('app');
    const error = await loaded.getData('/index.html').catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(errorCode(error)).toBe<ErrorCode>('core.checksum_mismatch');

    // Untouched entries still read.
    expect(await loaded.getData('/app.js')).toEqual(Buffer.from('console.log("app");', 'utf8'));
  });

  it('does not verify entry checksums when dataRead.checksum.verify is false', async () => {
    const source = makeSource({ dataRead: { checksum: { verify: false } } });
    await install(source, '1.0.0', '<h1>v1</h1>');
    await corruptChecksum(source, '/index.html');

    const loaded = await source.load('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));
  });

  it('verifies the header checksum on load by default', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await corruptHeaderChecksum(source);

    const error = await source.load('app').catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(errorCode(error)).toBe<ErrorCode>('core.invalid_header_checksum');
  });

  it('loads a header-corrupted bundle when headerRead.checksum.verify is false', async () => {
    const source = makeSource({ headerRead: { checksum: { verify: false } } });
    await install(source, '1.0.0', '<h1>v1</h1>');
    await corruptHeaderChecksum(source);

    const loaded = await source.load('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));
  });

  it('handles many concurrent loads and reads without hanging', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');

    const results = await Promise.all(
      Array.from({ length: 50 }, async () => {
        const loaded = await source.load('app');
        return loaded.getData('/index.html');
      })
    );
    for (const data of results) {
      expect(data).toEqual(Buffer.from('<h1>v1</h1>', 'utf8'));
    }
  });

  it('removes a staged version but protects the current one', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await stage(source, '1.1.0', '<h1>v2</h1>');

    expect(await source.removeRemoteBundle('app', '1.1.0')).toEqual({
      name: 'app',
      version: '1.1.0',
      kind: 'removed',
    });
    expect(
      await fs.access(source.getRemoteBundleFilepath('app', '1.1.0')).catch(() => 'gone')
    ).toBe('gone');
    expect(await source.removeRemoteBundle('app', '1.1.0')).toMatchObject({
      kind: 'version_not_exists',
    });
    expect(await source.removeRemoteBundle('app', '1.0.0')).toMatchObject({ kind: 'in_use' });
    expect(await source.removeRemoteBundle('app', '1.0.0', true)).toMatchObject({
      kind: 'removed',
    });
    expect(await source.removeRemoteBundle('missing', '1.0.0')).toMatchObject({
      kind: 'not_exists',
    });
  });

  it('removes several versions at once', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await stage(source, '1.1.0', '<h1>v2</h1>');
    await stage(source, '1.2.0', '<h1>v3</h1>');

    const results = await source.removeRemoteBundles({
      app: { versions: ['1.1.0', '1.2.0'] },
    });
    expect(results.map(x => [x.version, x.kind])).toEqual([
      ['1.1.0', 'removed'],
      ['1.2.0', 'removed'],
    ]);
    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '1.0.0' });
  });

  it('prunes the versions that are neither current, previous nor staged', async () => {
    const source = makeSource();
    await install(source, '1.0.0', '<h1>v1</h1>');
    await install(source, '1.1.0', '<h1>v2</h1>');
    await install(source, '1.2.0', '<h1>v3</h1>');
    await stage(source, '1.3.0', '<h1>v4</h1>');

    // current = 1.2.0, previous = 1.1.0, staged = 1.3.0; 1.0.0 is prunable.
    expect(await source.pruneRemoteBundle('app')).toEqual({
      name: 'app',
      prunedVersions: ['1.0.0'],
    });
    expect(
      await fs.access(source.getRemoteBundleFilepath('app', '1.0.0')).catch(() => 'gone')
    ).toBe('gone');
    // Idempotent once nothing else is prunable.
    expect(await source.pruneRemoteBundle('app')).toEqual({ name: 'app', prunedVersions: [] });
    expect(await source.pruneRemoteBundles(['app', 'missing'])).toEqual([
      { name: 'app', prunedVersions: [] },
      { name: 'missing', prunedVersions: [] },
    ]);
  });
});
