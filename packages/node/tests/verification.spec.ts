import { Buffer } from 'node:buffer';
import { randomBytes, webcrypto } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { type ServerType, serve } from '@hono/node-server';
import { makeIntegrity, signSignature } from '@wvb/config/remote';
import getPort from 'get-port';
import { Hono } from 'hono';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import {
  BundleBuilder,
  BundleSource,
  computeIntegrity,
  type ErrorCode,
  isWebviewBundleError,
  parseIntegrity,
  Remote,
  Updater,
  writeBundleIntoBuffer,
} from '../dist/index.js';

const { subtle } = webcrypto as unknown as Crypto;

let port: number;
let server: ServerType;

// ed25519 keypair (WebCrypto, same stack the producer uses for signing).
let pkcs8PrivateKey: Buffer; // signer input (format: 'pkcs8')
let spkiPublicKeyDer: Buffer; // verifier input (format: 'spkiDer')

interface ServedBundle {
  buf: Buffer;
  integrity: string;
  signature: string;
}

// name -> version -> served bundle (bytes + producer-generated metadata)
const served: Record<string, Record<string, ServedBundle>> = {};

function buildBundleBuf(name: string, version: string): Buffer {
  const builder = new BundleBuilder();
  builder.insertEntry('/index.html', Buffer.from(`<h1>${name}@${version}</h1>`, 'utf8'));
  const bundle = builder.build();
  return Buffer.from(writeBundleIntoBuffer(bundle));
}

async function publish(
  name: string,
  version: string,
  opts: { tamperSignature?: boolean; wrongIntegrity?: boolean } = {}
): Promise<ServedBundle> {
  const buf = buildBundleBuf(name, version);
  // integrity over the EXACT served bytes (base64 SHA-2)
  let integrity = await makeIntegrity({ algorithm: 'sha256' }, buf);
  if (opts.wrongIntegrity) {
    integrity = await makeIntegrity(
      { algorithm: 'sha256' },
      Buffer.concat([buf, Buffer.from('x')])
    );
  }
  // signature over the integrity string (base64 ed25519)
  let signature = await signSignature(
    { algorithm: 'ed25519', key: { format: 'pkcs8', data: pkcs8PrivateKey } },
    Buffer.from(integrity, 'utf8')
  );
  if (opts.tamperSignature) {
    // flip the last base64 char to a different valid one -> still 64 bytes, wrong sig
    const last = signature.endsWith('A') ? 'B' : 'A';
    signature = signature.slice(0, -1) + last;
  }
  const entry: ServedBundle = { buf, integrity, signature };
  served[name] ??= {};
  served[name][version] = entry;
  return entry;
}

function respond(entry: ServedBundle, name: string, version: string): Response {
  const headers = new Headers();
  headers.set('content-type', 'application/webview-bundle');
  headers.set('webview-bundle-name', name);
  headers.set('webview-bundle-version', version);
  headers.set('webview-bundle-integrity', entry.integrity);
  headers.set('webview-bundle-signature', entry.signature);
  return new Response(new Uint8Array(entry.buf), { status: 200, headers });
}

beforeAll(async () => {
  const kp = (await subtle.generateKey(
    { name: 'Ed25519' } as unknown as AlgorithmIdentifier,
    true,
    ['sign', 'verify']
  )) as CryptoKeyPair;
  pkcs8PrivateKey = Buffer.from(await subtle.exportKey('pkcs8', kp.privateKey));
  spkiPublicKeyDer = Buffer.from(await subtle.exportKey('spki', kp.publicKey));

  port = await getPort();
  const app = new Hono();
  app.get('/bundles/:name/:version', c => {
    const name = c.req.param('name');
    const version = c.req.param('version');
    const entry = served[name]?.[version];
    if (entry == null) return c.notFound();
    return respond(entry, name, version);
  });
  app.get('/bundles/:name', c => {
    const name = c.req.param('name');
    const versions = served[name];
    if (versions == null) return c.notFound();
    const version = Object.keys(versions).at(-1) as string;
    return respond(versions[version]!, name, version);
  });
  server = serve({ fetch: app.fetch, port });
});

afterAll(() => {
  return new Promise<void>((resolve, reject) => {
    if (server == null) return resolve();
    server.close(e => (e != null ? reject(e) : resolve()));
  });
});

describe('integrity (compute / parse)', () => {
  it('computes the string core computes', () => {
    // Same vector as core's own integrity_serialize test.
    const integrity = computeIntegrity('sha256', Buffer.from('test', 'utf8')).serialize();
    expect(integrity).toBe('sha256:n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=');
  });

  it('agrees with the producer helper on every algorithm', async () => {
    const data = randomBytes(256);
    for (const algorithm of ['sha256', 'sha384', 'sha512'] as const) {
      expect(computeIntegrity(algorithm, data).serialize()).toBe(
        await makeIntegrity({ algorithm }, data)
      );
    }
  });

  it('round-trips through parse and validates the right bytes only', () => {
    const data = Buffer.from('<h1>hello</h1>', 'utf8');
    const serialized = computeIntegrity('sha384', data).serialize();
    const parsed = parseIntegrity(serialized);

    expect(parsed.serialize()).toBe(serialized);
    expect(parsed.value()).toHaveLength(48); // sha384 = 48 bytes
    expect(parsed.validate(data)).toBe(true);
    expect(parsed.validate(Buffer.from('tampered', 'utf8'))).toBe(false);
  });

  it('rejects a malformed integrity string as a webview-bundle error', () => {
    expect(() => parseIntegrity('not-an-integrity')).toThrow(
      expect.objectContaining({ code: 'core.invalid_integrity' })
    );
  });

  it('produces a string that validates the bundle bytes it was computed over', () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>hello</h1>', 'utf8'));
    const buf = writeBundleIntoBuffer(builder.build());

    expect(parseIntegrity(computeIntegrity('sha256', buf).serialize()).validate(buf)).toBe(true);
  });
});

describe('integrity + signature verification (producer -> core)', () => {
  let tmpdir: string;
  let builtinDir: string;
  let remoteDir: string;

  beforeEach(async () => {
    tmpdir = path.join(os.tmpdir(), 'wvb-node-intsig', randomBytes(8).toString('hex'));
    builtinDir = path.join(tmpdir, 'builtin');
    remoteDir = path.join(tmpdir, 'remote');
    await fs.mkdir(builtinDir, { recursive: true });
    await fs.mkdir(remoteDir, { recursive: true });
    for (const k of Object.keys(served)) delete served[k];
  });

  afterEach(async () => {
    try {
      await fs.rm(tmpdir, { recursive: true });
    } catch {}
  });

  function setup() {
    const source = new BundleSource({ builtinDir, remoteDir });
    const remote = new Remote(`http://localhost:${port}`);
    const updater = new Updater(source, remote, {
      integrityPolicy: 'strict',
      signatureVerifier: {
        algorithm: 'ed25519',
        key: { format: 'spkiDer', data: spkiPublicKeyDer },
      },
    });
    return { source, remote, updater };
  }

  it('download verifies integrity+signature over served bytes (strict)', async () => {
    await publish('app', '0.2.0');
    const { updater } = setup();
    const info = await updater.download('app', '0.2.0');
    expect(info.version).toBe('0.2.0');
    expect(info.integrity).toMatch(/^sha256:/);
    expect(info.signature).toBeTruthy();
  });

  it('install re-verifies the staged bundle and activates it', async () => {
    await publish('app', '0.2.0');
    const { source, updater } = setup();
    await updater.download('app', '0.2.0');
    await updater.install('app', '0.2.0');
    expect(await source.loadVersion('app')).toEqual({ type: 'remote', version: '0.2.0' });
    const loaded = await source.loadDescriptor('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>app@0.2.0</h1>', 'utf8'));
  });

  it('rejects a download whose integrity does not match the bytes', async () => {
    await publish('app', '0.2.0', { wrongIntegrity: true });
    const { updater } = setup();
    await expect(updater.download('app', '0.2.0')).rejects.toThrowError();
  });

  it('rejects a download whose signature is invalid', async () => {
    await publish('app', '0.2.0', { tamperSignature: true });
    const { updater } = setup();
    await expect(updater.download('app', '0.2.0')).rejects.toThrowError();
  });

  it('loadDescriptor rejects a remote bundle whose recorded integrity is wrong; policy off loads it', async () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>remote</h1>', 'utf8'));
    const bundle = builder.build();
    // A well-formed sha256 integrity over unrelated bytes: it parses, then fails to match the file.
    const wrongIntegrity = await makeIntegrity(
      { algorithm: 'sha256' },
      Buffer.from('not the bundle bytes')
    );

    const source = new BundleSource({ builtinDir, remoteDir });
    await source.writeRemoteBundle('app', '1.0.0', bundle, { integrity: wrongIntegrity });
    await source.updateRemoteVersion('app', '1.0.0');

    // Default config verifies remote bundles under the 'optional' policy: a present-but-wrong
    // integrity is rejected on load.
    const error = await source.loadDescriptor('app').catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(error.code).toBe<ErrorCode>('core.integrity_verify_failed');

    const off = new BundleSource({ builtinDir, remoteDir, integrity: { policy: 'off' } });
    const loaded = await off.loadDescriptor('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>remote</h1>', 'utf8'));
  });

  it("checkMode 'all' rejects an unhashed builtin on load; the default 'onlyRemote' serves it", async () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>builtin</h1>', 'utf8'));
    const bundleBuf = Buffer.from(writeBundleIntoBuffer(builder.build()));
    await fs.mkdir(path.join(builtinDir, 'app'), { recursive: true });
    await fs.writeFile(path.join(builtinDir, 'app', 'app_1.0.0.wvb'), bundleBuf);
    await fs.writeFile(
      path.join(builtinDir, 'manifest.json'),
      JSON.stringify({
        manifestVersion: 1,
        entries: { app: { versions: { '1.0.0': {} }, currentVersion: '1.0.0' } },
      })
    );

    // The builtin manifest carries no integrity string, so strict verification of builtin
    // bundles must refuse to load it.
    const strict = new BundleSource({
      builtinDir,
      remoteDir,
      integrity: { policy: 'strict', checkMode: 'all' },
    });
    const error = await strict.loadDescriptor('app').catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(error.code).toBe<ErrorCode>('core.integrity_verify_failed');

    // The default 'onlyRemote' mode leaves builtin bundles alone, so the same bundle loads.
    const source = new BundleSource({ builtinDir, remoteDir });
    const loaded = await source.loadDescriptor('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>builtin</h1>', 'utf8'));
  });
});
