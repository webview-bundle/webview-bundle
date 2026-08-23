import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';
import fs from 'node:fs/promises';
import type { AddressInfo } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { serve as honoServe, type ServerType } from '@hono/node-server';
import type { ManifestData } from '@wvb/node';
import { BundleBuilder, readBundle, writeBundleIntoBuffer } from '@wvb/node';
import { Hono } from 'hono';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { pathExists } from '../fs.js';
import type { Logger } from '../log.js';
import { builtin } from './builtin.js';
import { pack } from './pack.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-builtin-'));
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

async function write(rel: string, content: string) {
  const file = path.join(root, rel);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, content);
}

async function writeWorkspace(
  dir: string,
  pkg: Record<string, unknown>,
  files: Record<string, string> = { 'dist/index.html': '<h1>Hello</h1>' }
) {
  await write(path.join(dir, 'package.json'), JSON.stringify(pkg));
  for (const [rel, content] of Object.entries(files)) {
    await write(path.join(dir, rel), content);
  }
}

async function readManifest(rel: string): Promise<ManifestData & { entries: Record<string, any> }> {
  const manifest = JSON.parse(await fs.readFile(path.join(root, rel), 'utf8')) as ManifestData;
  Object.defineProperty(manifest, 'entries', { enumerable: false, get: () => manifest.bundles });
  return manifest as ManifestData & { entries: Record<string, any> };
}

function createTestLogger() {
  const messages: string[] = [];
  const record = (message: string) => {
    messages.push(message);
  };
  const logger = { debug: record, info: record, warn: record, error: record } as unknown as Logger;
  return { logger, messages };
}

async function generateEd25519Key() {
  const keyPair = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, [
    'sign',
    'verify',
  ])) as CryptoKeyPair;
  return {
    publicKey: keyPair.publicKey,
    privateKey: Buffer.from(await crypto.subtle.exportKey('pkcs8', keyPair.privateKey)),
  };
}

describe('builtin with a local target', () => {
  it('installs each workspace bundle as "<dir>/<name>/<name>_<version>.wvb"', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    expect(result.manifest.entries['app-a']?.currentVersion).toBe('1.0.0');
    const installed = await readBundle(path.join(root, 'builtin/app-a/app-a_1.0.0.wvb'));
    expect(installed.getData('/index.html')?.toString('utf8')).toBe('<h1>Hello</h1>');
  });

  it('writes the manifest alongside the installed bundles', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    const manifest = await readManifest('builtin/manifest.json');
    expect(manifest.manifestVersion).toBe(1);
    expect(Object.keys(manifest.entries)).toEqual(['app-a']);
  });

  it('installs every matching workspace', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await writeWorkspace('workspaces/app-b', { name: 'app-b', version: '2.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    expect(result.manifest.entries['app-a']?.currentVersion).toBe('1.0.0');
    expect(result.manifest.entries['app-b']?.currentVersion).toBe('2.0.0');
  });

  it('strips the scope from the package name', async () => {
    await writeWorkspace('workspaces/app-a', { name: '@scope/app-a', version: '1.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-a']);
  });

  it('honors an explicit bundle name and version on the target', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    const result = await builtin({
      target: {
        type: 'local',
        workspaces: ['workspaces/*'],
        bundleName: 'renamed',
        version: '9.9.9',
      },
      dir: './builtin',
      cwd: root,
    });

    expect(result.manifest.entries.renamed?.currentVersion).toBe('9.9.9');
    expect(await pathExists(path.join(root, 'builtin/renamed/renamed_9.9.9.wvb'))).toBe(true);
  });

  it('resolves the workspace list from a function', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: async () => ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-a']);
  });

  it('writes an empty manifest when no workspace matches', async () => {
    const { logger, messages } = createTestLogger();

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      logger,
    });

    expect(result.manifest.entries).toEqual({});
    expect(await readManifest('builtin/manifest.json')).toEqual({
      manifestVersion: 1,
      entries: {},
    });
    expect(messages).toContain('No local workspaces to install.');
  });

  it('records the packed archive mtime as lastModified', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    const packed = await fs.stat(path.join(root, 'workspaces/app-a/.wvb/app-a.wvb'));
    expect(result.manifest.entries['app-a']?.versions['1.0.0']?.lastModified).toBe(
      packed.mtime.toUTCString()
    );
  });

  it('does not touch the disk when write is false', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      write: false,
    });

    expect(result.manifest.entries['app-a']?.currentVersion).toBe('1.0.0');
    expect(result.manifest.entries['app-a']?.versions['1.0.0']?.lastModified).toBeUndefined();
    expect(await pathExists(path.join(root, 'builtin'))).toBe(false);
    expect(await pathExists(path.join(root, 'workspaces/app-a/.wvb'))).toBe(false);
  });

  it('removes the install directory before installing', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await write('builtin/stale.wvb', 'stale');

    await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    expect(await pathExists(path.join(root, 'builtin/stale.wvb'))).toBe(false);
  });

  it('keeps the existing install directory when clean is false', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await write('builtin/stale.wvb', 'stale');

    await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      clean: false,
    });

    expect(await fs.readFile(path.join(root, 'builtin/stale.wvb'), 'utf8')).toBe('stale');
  });

  it('installs only the bundles matching the include patterns', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await writeWorkspace('workspaces/app-b', { name: 'app-b', version: '2.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      include: ['app-a'],
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-a']);
  });

  it('skips the bundles matching the exclude patterns', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await writeWorkspace('workspaces/app-b', { name: 'app-b', version: '2.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      exclude: ['app-b'],
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-a']);
  });

  it('accepts a predicate as an include filter', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await writeWorkspace('workspaces/app-b', { name: 'app-b', version: '2.0.0' });

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      include: [({ version }) => version === '2.0.0'],
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-b']);
  });

  it('packs the workspace before installing by default', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });

    await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    const packed = await readBundle(path.join(root, 'workspaces/app-a/.wvb/app-a.wvb'));
    expect(Object.keys(packed.descriptor().index().entries())).toEqual(['/index.html']);
  });

  it('reads the already packed archive when packBeforeInstall is false', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
    await write('seeded/seeded.txt', 'from the seeded archive');
    await pack({
      srcDir: './seeded',
      outFile: './workspaces/app-a/.wvb/app-a',
      cwd: root,
    });

    await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'], packBeforeInstall: false },
      dir: './builtin',
      cwd: root,
    });

    const installed = await readBundle(path.join(root, 'builtin/app-a/app-a_1.0.0.wvb'));
    expect(Object.keys(installed.descriptor().index().entries())).toEqual(['/seeded.txt']);
  });

  it('throws when the workspace package.json has no name', async () => {
    await writeWorkspace('workspaces/app-a', { version: '1.0.0' });

    await expect(
      builtin({
        target: { type: 'local', workspaces: ['workspaces/*'] },
        dir: './builtin',
        cwd: root,
      })
    ).rejects.toThrow('Out file is not specified');
  });

  it('throws when the workspace package.json has no version', async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a' });

    await expect(
      builtin({
        target: { type: 'local', workspaces: ['workspaces/*'] },
        dir: './builtin',
        cwd: root,
      })
    ).rejects.toThrow('Version is required for this operation');
  });
});

describe('builtin integrity and signature for a local target', () => {
  beforeEach(async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
  });

  it('computes a sha256 integrity over the installed bytes by default', async () => {
    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    const installed = await fs.readFile(path.join(root, 'builtin/app-a/app-a_1.0.0.wvb'));
    expect(result.manifest.entries['app-a']?.versions['1.0.0']?.integrity).toBe(
      `sha256:${createHash('sha256').update(installed).digest('base64')}`
    );
  });

  it('honors a custom integrity algorithm', async () => {
    const result = await builtin({
      target: {
        type: 'local',
        workspaces: ['workspaces/*'],
        integrity: { algorithm: 'sha512' },
      },
      dir: './builtin',
      cwd: root,
    });

    const installed = await fs.readFile(path.join(root, 'builtin/app-a/app-a_1.0.0.wvb'));
    expect(result.manifest.entries['app-a']?.versions['1.0.0']?.integrity).toBe(
      `sha512:${createHash('sha512').update(installed).digest('base64')}`
    );
  });

  it('omits the integrity when integrity is false', async () => {
    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'], integrity: false },
      dir: './builtin',
      cwd: root,
    });

    expect(result.manifest.entries['app-a']?.versions['1.0.0']?.integrity).toBeUndefined();
  });

  it('signs the integrity string of each installed bundle', async () => {
    const { publicKey, privateKey } = await generateEd25519Key();

    const result = await builtin({
      target: {
        type: 'local',
        workspaces: ['workspaces/*'],
        signature: { algorithm: 'ed25519', key: { format: 'pkcs8', data: privateKey } },
      },
      dir: './builtin',
      cwd: root,
    });

    const version = result.manifest.entries['app-a']?.versions['1.0.0'];
    const verified = await crypto.subtle.verify(
      { name: 'Ed25519' },
      publicKey,
      new Uint8Array(Buffer.from(version!.signature!, 'base64')),
      new Uint8Array(Buffer.from(version!.integrity!, 'utf8'))
    );
    expect(verified).toBe(true);
  });

  it('throws when a signature is configured while integrity is disabled', async () => {
    const { privateKey } = await generateEd25519Key();

    await expect(
      builtin({
        target: {
          type: 'local',
          workspaces: ['workspaces/*'],
          integrity: false,
          signature: { algorithm: 'ed25519', key: { format: 'pkcs8', data: privateKey } },
        },
        dir: './builtin',
        cwd: root,
      })
    ).rejects.toThrow('Cannot make signature without integrity');
  });
});

describe('builtin with a remote target', () => {
  let server: ServerType;
  let endpoint: string;
  let listedBundles: Array<{ name: string; version: string }>;
  let listRequestUrls: string[];

  beforeEach(async () => {
    listedBundles = [{ name: 'app-a', version: '1.0.0' }];
    listRequestUrls = [];

    const app = new Hono();
    app.get('/bundles', c => {
      listRequestUrls.push(c.req.url);
      return c.json(listedBundles);
    });
    app.get('/bundles/:name', c => {
      const name = c.req.param('name');
      const listed = listedBundles.find(x => x.name === name);
      if (listed == null) {
        return c.notFound();
      }
      const builder = new BundleBuilder();
      builder.insertEntry('/index.html', Buffer.from(`<h1>${name}</h1>`, 'utf8'));
      const data = writeBundleIntoBuffer(builder.build());
      return c.body(new Uint8Array(data), 200, {
        'content-type': 'application/webview-bundle',
        'webview-bundle-name': name,
        'webview-bundle-version': listed.version,
        'webview-bundle-integrity': `sha256:${createHash('sha256').update(data).digest('base64')}`,
        'webview-bundle-signature': 'c2lnbmF0dXJl',
        etag: `W/"${name}-${listed.version}"`,
        'last-modified': 'Wed, 21 Oct 2015 07:28:00 GMT',
      });
    });

    server = honoServe({ fetch: app.fetch, hostname: '127.0.0.1', port: 0 });
    if (!server.listening) {
      await new Promise<void>(resolve => {
        server.once('listening', () => resolve());
      });
    }
    endpoint = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  });

  afterEach(async () => {
    await new Promise<void>(resolve => {
      server.close(() => resolve());
    });
  });

  it('downloads every listed bundle into "<dir>/<name>/<name>_<version>.wvb"', async () => {
    const result = await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
    });

    expect(result.manifest.entries['app-a']?.currentVersion).toBe('1.0.0');
    const installed = await readBundle(path.join(root, 'builtin/app-a/app-a_1.0.0.wvb'));
    expect(installed.getData('/index.html')?.toString('utf8')).toBe('<h1>app-a</h1>');
  });

  it('records the metadata the remote reported in the manifest', async () => {
    const result = await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
    });

    const installed = await fs.readFile(path.join(root, 'builtin/app-a/app-a_1.0.0.wvb'));
    expect(result.manifest.entries['app-a']?.versions['1.0.0']).toEqual({
      etag: 'W/"app-a-1.0.0"',
      integrity: `sha256:${createHash('sha256').update(installed).digest('base64')}`,
      signature: 'c2lnbmF0dXJl',
      lastModified: 'Wed, 21 Oct 2015 07:28:00 GMT',
    });
  });

  it('forwards the channel when listing bundles', async () => {
    await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
      channel: 'beta',
    });

    expect(listRequestUrls).toEqual([`${endpoint}/bundles?channel=beta`]);
  });

  it('downloads only the bundles matching the include patterns', async () => {
    listedBundles = [
      { name: 'app-a', version: '1.0.0' },
      { name: 'app-b', version: '2.0.0' },
    ];

    const result = await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
      include: [/^app-a$/],
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-a']);
  });

  it('skips the bundles matching the exclude patterns', async () => {
    listedBundles = [
      { name: 'app-a', version: '1.0.0' },
      { name: 'app-b', version: '2.0.0' },
    ];

    const result = await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
      exclude: ['app-b'],
    });

    expect(Object.keys(result.manifest.entries)).toEqual(['app-a']);
  });

  it('writes an empty manifest when the remote lists no bundles', async () => {
    listedBundles = [];
    const { logger, messages } = createTestLogger();

    const result = await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
      logger,
    });

    expect(result.manifest.entries).toEqual({});
    expect(messages).toContain('No remote bundles to install.');
  });

  it('does not touch the disk when write is false', async () => {
    const result = await builtin({
      target: { type: 'remote', endpoint },
      dir: './builtin',
      cwd: root,
      write: false,
    });

    expect(result.manifest.entries['app-a']?.currentVersion).toBe('1.0.0');
    expect(await pathExists(path.join(root, 'builtin'))).toBe(false);
  });

  it('throws when the endpoint is missing', async () => {
    await expect(
      builtin({ target: { type: 'remote' }, dir: './builtin', cwd: root })
    ).rejects.toThrow('Remote endpoint is required.');
  });
});

describe('builtin mobile integration', () => {
  beforeEach(async () => {
    await writeWorkspace('workspaces/app-a', { name: 'app-a', version: '1.0.0' });
  });

  it('reports "ok" when the Android module keeps wvb assets uncompressed', async () => {
    await write(
      'android/build.gradle.kts',
      'android {\n  androidResources { noCompress += "wvb" }\n}\n'
    );

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      android: { dir: path.join(root, 'android'), checkNoCompress: true },
    });

    expect(result.android?.noCompressStatus).toBe('ok');
  });

  it('warns when the Android module re-compresses wvb assets', async () => {
    await write('android/build.gradle.kts', 'android {\n  namespace = "dev.wvb.app"\n}\n');
    const { logger, messages } = createTestLogger();

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      logger,
      android: { dir: path.join(root, 'android'), checkNoCompress: true },
    });

    expect(result.android?.noCompressStatus).toBe('missing');
    expect(messages.some(x => x.includes('noCompress += "wvb"'))).toBe(true);
  });

  it('leaves the Android status unset when the check is not requested', async () => {
    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
      android: { dir: path.join(root, 'android') },
    });

    expect(result.android?.noCompressStatus).toBeUndefined();
  });

  it('adds a folder reference for the install directory to Project.swift', async () => {
    await write('ios/Project.swift', 'let project = Project(\n  resources: [\n  ]\n)\n');

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './ios/bundles',
      cwd: root,
      ios: { dir: path.join(root, 'ios'), addProjectFolderReference: true },
    });

    expect(result.ios?.addFolderReferenceStatus).toBe('added');
    expect(await fs.readFile(path.join(root, 'ios/Project.swift'), 'utf8')).toContain(
      '.folderReference(path: "./bundles")'
    );
  });

  it('reports "already" when Project.swift references the install directory', async () => {
    await write(
      'ios/Project.swift',
      'let project = Project(\n  resources: [\n    .folderReference(path: "./bundles"),\n  ]\n)\n'
    );

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './ios/bundles',
      cwd: root,
      ios: { dir: path.join(root, 'ios'), addProjectFolderReference: true },
    });

    expect(result.ios?.addFolderReferenceStatus).toBe('already');
  });

  it('warns when Project.swift cannot be found', async () => {
    const { logger, messages } = createTestLogger();

    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './ios/bundles',
      cwd: root,
      logger,
      ios: { dir: path.join(root, 'ios'), addProjectFolderReference: true },
    });

    expect(result.ios?.addFolderReferenceStatus).toBe('not-found');
    expect(messages.some(x => x.includes('Project.swift not found'))).toBe(true);
  });

  it('omits the mobile results when neither platform is configured', async () => {
    const result = await builtin({
      target: { type: 'local', workspaces: ['workspaces/*'] },
      dir: './builtin',
      cwd: root,
    });

    expect(result.android).toBeUndefined();
    expect(result.ios).toBeUndefined();
  });
});
