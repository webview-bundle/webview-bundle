import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import type { BaseRemoteUploader, RemoteUploadParams } from '@wvb/config/remote';
import { type Bundle, BundleBuilder, writeBundle, writeBundleIntoBuffer } from '@wvb/node';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { remoteUpload } from './upload.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-upload-'));
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

function buildBundle(
  entries: Record<string, string> = { '/index.html': '<h1>Hello</h1>' }
): Bundle {
  const builder = new BundleBuilder();
  for (const [entryPath, content] of Object.entries(entries)) {
    builder.insertEntry(entryPath, Buffer.from(content, 'utf8'));
  }
  return builder.build();
}

async function writeBundleFile(rel: string, bundle: Bundle = buildBundle()) {
  const file = path.join(root, rel);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await writeBundle(bundle, file);
  return file;
}

function createTestUploader() {
  const uploaded: RemoteUploadParams[] = [];
  const uploader: BaseRemoteUploader = {
    async upload(params) {
      uploaded.push(params);
    },
  };
  return { uploader, uploaded };
}

describe('remoteUpload', () => {
  it('uploads the archive bytes read from a file path', async () => {
    const bundle = buildBundle();
    await writeBundleFile('app.wvb', bundle);
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({
      file: './app.wvb',
      bundleName: 'app',
      version: '1.0.0',
      uploader,
      cwd: root,
    });

    expect(uploaded).toHaveLength(1);
    expect(uploaded[0]?.bundleName).toBe('app');
    expect(uploaded[0]?.version).toBe('1.0.0');
    expect(uploaded[0]?.bundle).toEqual(writeBundleIntoBuffer(bundle));
  });

  it('accepts a Bundle object instead of a file path', async () => {
    const bundle = buildBundle();
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({ file: bundle, bundleName: 'app', version: '1.0.0', uploader });

    expect(uploaded[0]?.bundle).toEqual(writeBundleIntoBuffer(bundle));
  });

  it('resolves a relative file path against cwd', async () => {
    await writeBundleFile('nested/app.wvb');
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({
      file: './nested/app.wvb',
      bundleName: 'app',
      version: '1.0.0',
      uploader,
      cwd: root,
    });

    expect(uploaded).toHaveLength(1);
  });

  it('requires the exact file name, without appending the ".wvb" extension', async () => {
    await writeBundleFile('app.wvb');
    const { uploader } = createTestUploader();

    await expect(
      remoteUpload({ file: './app', bundleName: 'app', version: '1.0.0', uploader, cwd: root })
    ).rejects.toThrow(`File does not exist: ${path.join(root, 'app')}`);
  });

  it('throws when the file does not exist', async () => {
    const { uploader } = createTestUploader();

    await expect(
      remoteUpload({
        file: './missing.wvb',
        bundleName: 'app',
        version: '1.0.0',
        uploader,
        cwd: root,
      })
    ).rejects.toThrow(`File does not exist: ${path.join(root, 'missing.wvb')}`);
  });

  it('forwards the force flag', async () => {
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({
      file: buildBundle(),
      bundleName: 'app',
      version: '1.0.0',
      uploader,
      force: true,
    });

    expect(uploaded[0]?.force).toBe(true);
  });
});

describe('remoteUpload integrity', () => {
  it('computes a sha256 integrity by default', async () => {
    const bundle = buildBundle();
    const expected = createHash('sha256').update(writeBundleIntoBuffer(bundle)).digest('base64');
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({ file: bundle, bundleName: 'app', version: '1.0.0', uploader });

    expect(uploaded[0]?.integrity).toBe(`sha256:${expected}`);
  });

  it('honors a custom integrity algorithm', async () => {
    const bundle = buildBundle();
    const expected = createHash('sha512').update(writeBundleIntoBuffer(bundle)).digest('base64');
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({
      file: bundle,
      bundleName: 'app',
      version: '1.0.0',
      uploader,
      integrity: { algorithm: 'sha512' },
    });

    expect(uploaded[0]?.integrity).toBe(`sha512:${expected}`);
  });

  it('omits the integrity when integrity is false', async () => {
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({
      file: buildBundle(),
      bundleName: 'app',
      version: '1.0.0',
      uploader,
      integrity: false,
    });

    expect(uploaded[0]?.integrity).toBeUndefined();
  });
});

describe('remoteUpload signature', () => {
  it('signs the integrity string with the configured key', async () => {
    const keyPair = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, [
      'sign',
      'verify',
    ])) as CryptoKeyPair;
    const privateKey = Buffer.from(await crypto.subtle.exportKey('pkcs8', keyPair.privateKey));
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({
      file: buildBundle(),
      bundleName: 'app',
      version: '1.0.0',
      uploader,
      signature: { algorithm: 'ed25519', key: { format: 'pkcs8', data: privateKey } },
    });

    const { integrity, signature } = uploaded[0]!;
    const verified = await crypto.subtle.verify(
      { name: 'Ed25519' },
      keyPair.publicKey,
      new Uint8Array(Buffer.from(signature!, 'base64')),
      new Uint8Array(Buffer.from(integrity!, 'utf8'))
    );
    expect(verified).toBe(true);
  });

  it('omits the signature when no signature config is given', async () => {
    const { uploader, uploaded } = createTestUploader();

    await remoteUpload({ file: buildBundle(), bundleName: 'app', version: '1.0.0', uploader });

    expect(uploaded[0]?.signature).toBeUndefined();
  });

  it('throws when a signature is requested while integrity is disabled', async () => {
    const keyPair = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, [
      'sign',
      'verify',
    ])) as CryptoKeyPair;
    const privateKey = Buffer.from(await crypto.subtle.exportKey('pkcs8', keyPair.privateKey));
    const { uploader, uploaded } = createTestUploader();

    await expect(
      remoteUpload({
        file: buildBundle(),
        bundleName: 'app',
        version: '1.0.0',
        uploader,
        integrity: false,
        signature: { algorithm: 'ed25519', key: { format: 'pkcs8', data: privateKey } },
      })
    ).rejects.toThrow('Cannot make signature without integrity');

    expect(uploaded).toHaveLength(0);
  });
});
