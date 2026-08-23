import { Buffer } from 'node:buffer';
import { randomBytes, webcrypto } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { makeIntegrity, signSignature } from '@wvb/config/remote';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it } from 'vitest';
import {
  BundleBuilder,
  computeIntegrity,
  type ErrorCode,
  isWebviewBundleError,
  type ManifestVersionData,
  parseIntegrity,
  Remote,
  type SignatureVerifyKey,
  Source,
  Updater,
  writeBundleIntoBuffer,
} from '../dist/index.js';
import { caught, errorCode } from './errors.js';
import { buildBundleData, startUpdateServer, type UpdateServer } from './update-server.js';

const { subtle } = webcrypto as unknown as Crypto;

describe('integrity (compute / parse)', () => {
  it('computes the string core computes', () => {
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
    expect(errorCode(caught(() => parseIntegrity('not-an-integrity')))).toBe<ErrorCode>(
      'core.invalid_integrity'
    );
  });

  it('produces a string that validates the bundle bytes it was computed over', () => {
    const builder = new BundleBuilder();
    builder.insertEntry('/index.html', Buffer.from('<h1>hello</h1>', 'utf8'));
    const buf = Buffer.from(writeBundleIntoBuffer(builder.build()));

    expect(parseIntegrity(computeIntegrity('sha256', buf).serialize()).validate(buf)).toBe(true);
  });
});

describe('update signature verification (producer -> core)', () => {
  let server: UpdateServer;
  let pkcs8PrivateKey: Buffer;
  let spkiPublicKeyDer: Buffer;
  let tmpdir: string;

  beforeAll(async () => {
    const kp = (await subtle.generateKey(
      { name: 'Ed25519' } as unknown as AlgorithmIdentifier,
      true,
      ['sign', 'verify']
    )) as CryptoKeyPair;
    pkcs8PrivateKey = Buffer.from(await subtle.exportKey('pkcs8', kp.privateKey));
    spkiPublicKeyDer = Buffer.from(await subtle.exportKey('spki', kp.publicKey));
    server = await startUpdateServer();
  });

  afterAll(() => server.close());

  beforeEach(async () => {
    server.bundles = [{ name: 'app', version: '1.0.0', data: buildBundleData('app', '1.0.0') }];
    server.signer = ({ body }) =>
      signSignature(
        { algorithm: 'ed25519', key: { format: 'pkcs8', data: pkcs8PrivateKey } },
        body
      );
    tmpdir = path.join(os.tmpdir(), 'wvb-node-signature', randomBytes(8).toString('hex'));
    await fs.mkdir(tmpdir, { recursive: true });
  });

  afterEach(async () => {
    try {
      await fs.rm(tmpdir, { recursive: true });
    } catch {}
  });

  function keySet(): SignatureVerifyKey {
    return {
      id: 'key-2026',
      verify: { algorithm: 'ed25519', key: { format: 'spki_der', data: spkiPublicKeyDer } },
    };
  }

  function makeUpdater(key: SignatureVerifyKey) {
    const source = new Source({ builtinDir: path.join(tmpdir, 'builtin'), remoteDir: tmpdir });
    const remote = new Remote({ baseUrl: server.baseUrl });
    const updater = new Updater(source, remote, path.join(tmpdir, 'update.json'), {
      signature: { keys: [key] },
    });
    return { source, updater };
  }

  it('verifies the signature the server puts on the update', async () => {
    const { updater } = makeUpdater(keySet());
    const update = await updater.getUpdate({ expectSignatureKeyId: 'key-2026' });

    expect(update?.bundles).toEqual([{ name: 'app', version: '1.0.0' }]);
    expect(server.lastRequest?.headers.get('wvb-expect-signature')).toBe(
      'key_id="key-2026", alg="ed25519"'
    );
  });

  it('rejects a signature made over other bytes', async () => {
    server.signer = ({ body }) =>
      signSignature(
        { algorithm: 'ed25519', key: { format: 'pkcs8', data: pkcs8PrivateKey } },
        Buffer.concat([body, Buffer.from('x')])
      );

    const { updater } = makeUpdater(keySet());
    const error = await updater.getUpdate({ expectSignatureKeyId: 'key-2026' }).catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(errorCode(error)).toBe<ErrorCode>('core.signature_verify_failed');
  });

  it('rejects a malformed signature', async () => {
    server.signer = () => 'not base64';

    const { updater } = makeUpdater(keySet());
    const error = await updater.getUpdate({ expectSignatureKeyId: 'key-2026' }).catch(e => e);
    expect(errorCode(error)).toBe<ErrorCode>('core.invalid_signature');
  });

  it('rejects an update the server left unsigned', async () => {
    server.signer = undefined;

    const { updater } = makeUpdater(keySet());
    const error = await updater.getUpdate({ expectSignatureKeyId: 'key-2026' }).catch(e => e);
    expect(errorCode(error)).toBe<ErrorCode>('core.expect_signature_not_found');
  });

  it('verifies with a key given as a function', async () => {
    const seen: Array<{ message: Uint8Array; signature: string }> = [];
    const remote = new Remote({ baseUrl: server.baseUrl });
    const resp = await remote.getUpdate({
      expectSignature: {
        id: 'key-2026',
        verify: async (message, signature) => {
          seen.push({ message, signature });
          return true;
        },
      },
    });

    expect(resp?.signature?.keyId).toBe('key-2026');
    expect(seen).toHaveLength(1);
    expect(JSON.parse(Buffer.from(seen[0]!.message).toString('utf8')).bundles).toEqual([
      { name: 'app', version: '1.0.0' },
    ]);
    expect(seen[0]?.signature).toBe(resp?.signature?.sig);
  });

  it('fails when the key given as a function refuses the signature', async () => {
    const remote = new Remote({ baseUrl: server.baseUrl });
    const error = await remote
      .getUpdate({ expectSignature: { id: 'key-2026', verify: async () => false } })
      .catch(e => e);
    expect(errorCode(error)).toBe<ErrorCode>('core.signature_verify_failed');
  });

  it('rejects a key whose data does not match its format', () => {
    const error = caught(() =>
      new Remote({ baseUrl: server.baseUrl }).getUpdate({
        expectSignature: {
          id: 'key-2026',
          verify: { algorithm: 'ed25519', key: { format: 'sec1', data: Buffer.alloc(32) } },
        },
      })
    );
    expect(errorCode(error)).toBe<ErrorCode>('invalid_signature_key');
  });
});

describe('integrity verification (producer -> core)', () => {
  let server: UpdateServer;
  let tmpdir: string;
  let builtinDir: string;
  let remoteDir: string;

  beforeAll(async () => {
    server = await startUpdateServer();
  });

  afterAll(() => server.close());

  beforeEach(async () => {
    server.signer = undefined;
    tmpdir = path.join(os.tmpdir(), 'wvb-node-integrity', randomBytes(8).toString('hex'));
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

  async function publish(version: string, integrity: 'matching' | 'wrong' | 'none') {
    const data = buildBundleData('app', version);
    server.bundles = [
      {
        name: 'app',
        version,
        data,
        integrity:
          integrity === 'none'
            ? undefined
            : await makeIntegrity(
                { algorithm: 'sha256' },
                integrity === 'matching' ? data : Buffer.concat([data, Buffer.from('x')])
              ),
      },
    ];
  }

  function setup() {
    const source = new Source({ builtinDir, remoteDir });
    const remote = new Remote({ baseUrl: server.baseUrl });
    const updater = new Updater(source, remote, path.join(remoteDir, 'update.json'), {
      integrity: { policy: 'strict' },
    });
    return { source, updater };
  }

  it('installs a staged bundle whose integrity matches the served bytes', async () => {
    await publish('1.0.0', 'matching');
    const { source, updater } = setup();

    const update = await updater.getUpdate();
    await updater.download(update?.bundles ?? []);
    expect(await updater.install([{ name: 'app' }])).toMatchObject([
      { result: { type: 'installed' } },
    ]);
    expect(await source.getVersion('app')).toEqual({ source: 'remote', version: '1.0.0' });
  });

  it('refuses to install a staged bundle whose integrity does not match', async () => {
    await publish('1.0.0', 'wrong');
    const { source, updater } = setup();

    const update = await updater.getUpdate();
    await updater.download(update?.bundles ?? []);
    expect(await updater.install([{ name: 'app' }])).toMatchObject([
      { result: { type: 'verify_failed' } },
    ]);
    expect(await source.getVersion('app')).toBeNull();
  });

  it('refuses to install an unhashed bundle under the strict policy', async () => {
    await publish('1.0.0', 'none');
    const { updater } = setup();

    const update = await updater.getUpdate();
    await updater.download(update?.bundles ?? []);
    expect(await updater.install([{ name: 'app' }])).toMatchObject([
      { result: { type: 'verify_failed' } },
    ]);
  });

  async function stageRemote(source: Source, version: string, data?: ManifestVersionData) {
    const filepath = source.getRemoteBundleFilepath('app', version);
    await fs.mkdir(path.dirname(filepath), { recursive: true });
    await fs.writeFile(filepath, buildBundleData('app', version));
    await source.stageRemoteBundle('app', { version, data });
    await source.updateRemoteVersion('app', version);
  }

  it('load rejects a remote bundle whose recorded integrity is wrong; policy off loads it', async () => {
    // A well-formed sha256 integrity over unrelated bytes: it parses, then fails to match the file.
    const wrongIntegrity = await makeIntegrity(
      { algorithm: 'sha256' },
      Buffer.from('not the bundle bytes')
    );

    const source = new Source({ builtinDir, remoteDir });
    await stageRemote(source, '1.0.0', { integrity: wrongIntegrity });

    // The default config verifies remote bundles under the 'optional' policy: a present-but-wrong
    // integrity is rejected on load.
    const error = await source.load('app').catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(errorCode(error)).toBe<ErrorCode>('core.integrity_verify_failed');

    const off = new Source({
      builtinDir,
      remoteDir,
      options: { integrity: { policy: 'off' } },
    });
    const loaded = await off.load('app');
    expect(await loaded.getData('/index.html')).toEqual(Buffer.from('<h1>app@1.0.0</h1>', 'utf8'));
  });

  it("checkMode 'all' rejects an unhashed builtin on load; the default 'only_remote' serves it", async () => {
    await fs.mkdir(path.join(builtinDir, 'app'), { recursive: true });
    await fs.writeFile(
      path.join(builtinDir, 'app', '1.0.0.wvb'),
      buildBundleData('app', 'builtin')
    );
    await fs.writeFile(
      path.join(builtinDir, 'manifest.json'),
      JSON.stringify({
        manifestVersion: 1,
        bundles: { app: { versions: { '1.0.0': {} }, currentVersion: '1.0.0' } },
      })
    );

    // The builtin manifest carries no integrity string, so strict verification of builtin
    // bundles must refuse to load it.
    const strict = new Source({
      builtinDir,
      remoteDir,
      options: { integrity: { policy: 'strict', checkMode: 'all' } },
    });
    const error = await strict.load('app').catch(e => e);
    expect(isWebviewBundleError(error)).toBe(true);
    expect(errorCode(error)).toBe<ErrorCode>('core.integrity_verify_failed');

    // The default 'only_remote' mode leaves builtin bundles alone, so the same bundle loads.
    const source = new Source({ builtinDir, remoteDir });
    const loaded = await source.load('app');
    expect(await loaded.getData('/index.html')).toEqual(
      Buffer.from('<h1>app@builtin</h1>', 'utf8')
    );
  });
});
