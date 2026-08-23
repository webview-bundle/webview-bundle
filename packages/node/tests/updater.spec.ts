import { Buffer } from 'node:buffer';
import { randomBytes } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import { Remote, Source, Updater, type UpdaterOptions } from '../dist/index.js';
import { errorCode } from './errors.js';
import { buildBundleData, startUpdateServer, type UpdateServer } from './update-server.js';

describe('updater', () => {
  let server: UpdateServer;
  let tmpdir: string;
  let builtinDir: string;
  let remoteDir: string;
  let updateFilepath: string;

  beforeAll(async () => {
    server = await startUpdateServer();
  });

  afterAll(() => server.close());

  beforeEach(async () => {
    server.bundles = [{ name: 'app', version: '1.0.0', data: buildBundleData('app', '1.0.0') }];
    server.createdAt = '2026-01-01T00:00:00Z';
    server.failWith = undefined;

    tmpdir = path.join(os.tmpdir(), 'webview-bundle-node-updater', randomBytes(8).toString('hex'));
    builtinDir = path.join(tmpdir, 'builtin');
    remoteDir = path.join(tmpdir, 'remote');
    updateFilepath = path.join(remoteDir, 'update.json');
    await fs.mkdir(builtinDir, { recursive: true });
    await fs.mkdir(remoteDir, { recursive: true });
  });

  afterEach(async () => {
    try {
      await fs.rm(tmpdir, { recursive: true });
    } catch {}
  });

  function setup(options?: UpdaterOptions) {
    const source = new Source({ builtinDir, remoteDir });
    const remote = new Remote({ baseUrl: server.baseUrl });
    const updater = new Updater(source, remote, updateFilepath, options);
    return { source, remote, updater };
  }

  it('lists the bundles the source is missing', async () => {
    const { updater } = setup();
    const update = await updater.getUpdate();

    expect(update?.bundles).toEqual([{ name: 'app', version: '1.0.0' }]);
    expect(update?.runtimeVersion).toBe(1);
    await expect(fs.readFile(updateFilepath, 'utf8')).resolves.toContain('"app"');
  });

  it('download stages a version and install activates it', async () => {
    const { source, updater } = setup();
    const update = await updater.getUpdate();

    const downloaded = await updater.download(update?.bundles ?? []);
    expect(downloaded).toEqual([{ name: 'app', version: '1.0.0', result: { type: 'downloaded' } }]);

    // A download stages the bundle but must NOT activate it.
    expect(await source.getRemoteStagedVersion('app')).toBe('1.0.0');
    expect(await source.getVersion('app')).toBeNull();

    const installed = await updater.install([{ name: 'app' }]);
    expect(installed).toEqual([
      { name: 'app', installVersion: '1.0.0', result: { type: 'installed' } },
    ]);

    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '1.0.0' });
    const loaded = await source.load('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>app@1.0.0</h1>', 'utf8'));
  });

  it('has nothing to update once every bundle is current', async () => {
    const { updater } = setup();
    const update = await updater.getUpdate();
    await updater.download(update?.bundles ?? []);
    await updater.install([{ name: 'app' }]);

    expect(await updater.getUpdate()).toBeNull();
  });

  it('reports an install of a version that was never staged', async () => {
    const { updater } = setup();
    expect(await updater.install([{ name: 'app' }])).toEqual([
      { name: 'app', result: { type: 'staged_bundle_not_exists' } },
    ]);

    const update = await updater.getUpdate();
    await updater.download(update?.bundles ?? []);
    expect(await updater.install([{ name: 'app', version: '9.9.9' }])).toEqual([
      {
        name: 'app',
        targetVersion: '9.9.9',
        result: { type: 'staged_version_not_matched' },
      },
    ]);
  });

  it('installs the targets which did not fail', async () => {
    const { source, updater } = setup();
    const update = await updater.getUpdate();
    await updater.download(update?.bundles ?? []);

    const installed = await updater.install([{ name: 'app' }, { name: 'docs' }]);
    expect(installed).toEqual([
      { name: 'app', installVersion: '1.0.0', result: { type: 'installed' } },
      { name: 'docs', result: { type: 'staged_bundle_not_exists' } },
    ]);
    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '1.0.0' });
    expect(await source.getVersion('docs')).toBeNull();
  });

  it('rollback restores the previous version', async () => {
    const { source, updater } = setup();
    const first = await updater.getUpdate();
    await updater.download(first?.bundles ?? []);
    await updater.install([{ name: 'app' }]);

    server.bundles = [{ name: 'app', version: '2.0.0', data: buildBundleData('app', '2.0.0') }];
    server.createdAt = '2026-02-01T00:00:00Z';
    const second = await updater.getUpdate();
    await updater.download(second?.bundles ?? []);
    await updater.install([{ name: 'app' }]);
    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '2.0.0' });

    expect(await updater.rollback([{ name: 'app' }])).toEqual([
      { name: 'app', rollbackVersion: '1.0.0', result: { type: 'rolled_back' } },
    ]);
    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '1.0.0' });
    const loaded = await source.load('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>app@1.0.0</h1>', 'utf8'));
  });

  it('reports a rollback with no previous version', async () => {
    const { updater } = setup();
    expect(await updater.rollback([{ name: 'app' }])).toEqual([
      { name: 'app', result: { type: 'previous_bundle_not_exists' } },
    ]);
  });

  it('reuses the stored update when the server answers not modified', async () => {
    const { updater } = setup();
    const first = await updater.getUpdate();
    const second = await updater.getUpdate();

    expect(server.lastRequest?.headers.get('if-none-match')).toBeTruthy();
    expect(second).toEqual(first);
  });

  it('refuses an update older than the stored one', async () => {
    const { updater } = setup();
    expect((await updater.getUpdate())?.bundles).toEqual([{ name: 'app', version: '1.0.0' }]);

    server.bundles = [{ name: 'app', version: '0.9.0', data: buildBundleData('app', '0.9.0') }];
    server.createdAt = '2025-01-01T00:00:00Z';

    expect((await updater.getUpdate())?.bundles).toEqual([{ name: 'app', version: '1.0.0' }]);
  });

  it('downloads several bundles and reports each result', async () => {
    server.bundles = [
      { name: 'app', version: '1.0.0', data: buildBundleData('app', '1.0.0') },
      { name: 'docs', version: '2.0.0', data: buildBundleData('docs', '2.0.0') },
    ];
    const { source, updater } = setup();
    const update = await updater.getUpdate();

    const downloaded = await updater.download(update?.bundles ?? [], { concurrency: 2 });
    expect(downloaded.map(x => x.result)).toEqual([{ type: 'downloaded' }, { type: 'downloaded' }]);

    expect(
      await updater.install([{ name: 'app' }, { name: 'docs', version: '2.0.0' }])
    ).toMatchObject([{ result: { type: 'installed' } }, { result: { type: 'installed' } }]);
    expect(await source.getVersion('docs')).toEqual({ source: 'remote', version: '2.0.0' });
  });

  it('reports the bundle that could not be downloaded', async () => {
    server.bundles = [
      {
        name: 'app',
        version: '1.0.0',
        data: buildBundleData('app', '1.0.0'),
        downloadUrl: `${server.baseUrl}/bundles/app/9.9.9`,
      },
    ];
    const { source, updater } = setup();
    const update = await updater.getUpdate();

    const downloaded = await updater.download(update?.bundles ?? []);
    expect(downloaded[0]?.result).toMatchObject({ type: 'error', code: 'core.remote_http' });
    expect(await source.getRemoteStagedVersion('app')).toBeNull();
  });

  it('rejects an update whose signature key set is unknown', async () => {
    const { updater } = setup();
    const error = await updater.getUpdate({ expectSignatureKeyId: 'missing' }).catch(e => e);
    expect(errorCode(error)).toBe('core.expect_signature_not_found');
  });
});
