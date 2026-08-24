import { assert, assertEquals, assertRejects, assertThrows } from '@std/assert';
import {
  Cancellation,
  Remote,
  type SignatureVerifyKey,
  Source,
  Updater,
  type UpdaterOptions,
  WebviewBundleError,
} from './mod.ts';
import {
  buildBundleData,
  ed25519Signer,
  loadTestLib,
  startUpdateServer,
  tempDir,
  type UpdateServer,
} from './testing.ts';

loadTestLib();

const UNREACHABLE = 'http://127.0.0.1:59999';

/** A source + remote + updater over a temp dir, all freed when the scope ends. */
function setup(server: UpdateServer, options?: UpdaterOptions) {
  const dir = tempDir('wvb-deno-updater-');
  const source = new Source({
    builtinDir: `${dir.path}/builtin`,
    remoteDir: `${dir.path}/remote`,
  });
  const remote = new Remote({ baseUrl: server.baseUrl });
  const updater = new Updater(source, remote, `${dir.path}/remote/update.json`, options);
  return {
    dir,
    source,
    remote,
    updater,
    [Symbol.dispose]: () => {
      updater.free();
      remote.free();
      source.free();
      dir[Symbol.dispose]();
    },
  };
}

function servedBundle(name: string, version: string) {
  return { name, version, data: buildBundleData(name, version) };
}

Deno.test('Remote rejects on an unreachable endpoint', async () => {
  using remote = new Remote({ baseUrl: UNREACHABLE, http: { connectTimeout: 2000 } });
  await assertRejects(() => remote.getUpdate());
});

Deno.test('Remote fails closed on a base url the core rejects', () => {
  assertThrows(() => new Remote({ baseUrl: 'not a url' }), WebviewBundleError);
});

Deno.test('Remote sends the configured http options on every request', async () => {
  await using server = startUpdateServer();
  using remote = new Remote({
    baseUrl: server.baseUrl,
    http: {
      defaultHeaders: { authorization: 'Bearer tok-123', 'x-tenant': 'acme' },
      userAgent: 'wvb-deno-test/1.0',
    },
  });
  await remote.getUpdate();

  assertEquals(server.lastRequest?.headers.get('authorization'), 'Bearer tok-123');
  assertEquals(server.lastRequest?.headers.get('x-tenant'), 'acme');
  assertEquals(server.lastRequest?.headers.get('user-agent'), 'wvb-deno-test/1.0');
});

Deno.test('Remote reads the update document and downloads a bundle file', async () => {
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  using remote = new Remote({ baseUrl: server.baseUrl });

  const response = await remote.getUpdate({ channel: 'beta' });
  assertEquals(response?.update.runtimeVersion, 1);
  assertEquals(response?.update.bundles, [{ name: 'app', version: '1.0.0' }]);
  assertEquals(response?.update.metadata, { channel: 'beta' });
  assert(response?.etag != null);

  // The same etag answers 304, which surfaces as "nothing new".
  assertEquals(await remote.getUpdate({ etag: response.etag, channel: 'beta' }), null);

  using dir = tempDir('wvb-deno-download-');
  const filepath = `${dir.path}/app.wvb`;
  await remote.download(`${server.baseUrl}/bundles/app/1.0.0`, filepath);
  assertEquals(Deno.statSync(filepath).isFile, true);
});

Deno.test('a cancelled download rejects with the shared code', async () => {
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  using remote = new Remote({ baseUrl: server.baseUrl });
  using cancellation = new Cancellation();
  using dir = tempDir('wvb-deno-cancel-');

  assertEquals(cancellation.isCancelled(), false);
  cancellation.cancel();
  assertEquals(cancellation.isCancelled(), true);

  const error = await assertRejects(
    () =>
      remote.download(`${server.baseUrl}/bundles/app/1.0.0`, `${dir.path}/app.wvb`, cancellation),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.cancelled');
});

Deno.test('Updater downloads, installs and rolls back a bundle', async () => {
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  using ctx = setup(server);

  const update = await ctx.updater.getUpdate();
  assertEquals(update?.bundles, [{ name: 'app', version: '1.0.0' }]);

  assertEquals(await ctx.updater.download(update?.bundles ?? []), [
    { name: 'app', version: '1.0.0', result: { type: 'downloaded' } },
  ]);
  // A download stages the bundle but must NOT activate it.
  assertEquals(await ctx.source.getRemoteStagedVersion('app'), '1.0.0');
  assertEquals(await ctx.source.getVersion('app'), null);

  assertEquals(await ctx.updater.install([{ name: 'app' }]), [
    { name: 'app', installVersion: '1.0.0', result: { type: 'installed' } },
  ]);
  assertEquals(await ctx.source.getVersion('app'), { source: 'remote', version: '1.0.0' });
  using loaded = await ctx.source.load('app');
  assertEquals(await loaded.getData('/index.html'), new TextEncoder().encode('<h1>app@1.0.0</h1>'));

  // A second version, so there is something to roll back to.
  server.bundles = [servedBundle('app', '1.1.0')];
  const next = await ctx.updater.getUpdate();
  await ctx.updater.download(next?.bundles ?? []);
  await ctx.updater.install([{ name: 'app' }]);
  assertEquals(await ctx.source.getVersion('app'), { source: 'remote', version: '1.1.0' });

  assertEquals(await ctx.updater.rollback([{ name: 'app' }]), [
    { name: 'app', rollbackVersion: '1.0.0', result: { type: 'rolled_back' } },
  ]);
  assertEquals(await ctx.source.getVersion('app'), { source: 'remote', version: '1.0.0' });
});

Deno.test('Updater reports a failed download per bundle instead of rejecting', async () => {
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  using ctx = setup(server);

  const results = await ctx.updater.download([{ name: 'app', version: '9.9.9' }]);
  assertEquals(results.length, 1);
  assertEquals(results[0].result.type, 'error');
  // Nothing was staged, so the source is untouched.
  assertEquals(await ctx.source.getRemoteStagedVersion('app'), null);
});

Deno.test('Updater has nothing to report once every bundle is current', async () => {
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  using ctx = setup(server);

  const update = await ctx.updater.getUpdate();
  await ctx.updater.download(update?.bundles ?? []);
  await ctx.updater.install([{ name: 'app' }]);

  assertEquals(await ctx.updater.getUpdate(), null);
});

Deno.test('Updater sends the configured channel', async () => {
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  using ctx = setup(server, { channel: 'beta' });

  const update = await ctx.updater.getUpdate();
  assertEquals(update?.metadata, { channel: 'beta' });
  assertEquals(server.lastRequest?.headers.get('wvb-update-channel'), 'beta');
});

Deno.test('Updater verifies a signed update against the configured key', async () => {
  const { publicKeyDer, sign } = await ed25519Signer();
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  server.signer = sign;

  const key: SignatureVerifyKey = {
    id: '2026-08',
    verify: { algorithm: 'ed25519', key: { format: 'spki_der', data: publicKeyDer } },
  };
  using ctx = setup(server, { signature: { keys: [key] } });

  const update = await ctx.updater.getUpdate({ expectSignatureKeyId: '2026-08' });
  assertEquals(update?.bundles, [{ name: 'app', version: '1.0.0' }]);
  assertEquals(
    server.lastRequest?.headers.get('wvb-expect-signature'),
    'key_id="2026-08", alg="ed25519"'
  );
});

Deno.test('Updater refuses an update signed by the wrong key', async () => {
  const server_signer = await ed25519Signer();
  const other = await ed25519Signer();
  await using server = startUpdateServer();
  server.bundles = [servedBundle('app', '1.0.0')];
  server.signer = server_signer.sign;

  using ctx = setup(server, {
    signature: {
      keys: [
        {
          id: '2026-08',
          verify: { algorithm: 'ed25519', key: { format: 'spki_der', data: other.publicKeyDer } },
        },
      ],
    },
  });

  const error = await assertRejects(
    () => ctx.updater.getUpdate({ expectSignatureKeyId: '2026-08' }),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.signature_verify_failed');
});

Deno.test('Updater rejects a key id it was not configured with', async () => {
  await using server = startUpdateServer();
  using ctx = setup(server);

  const error = await assertRejects(
    () => ctx.updater.getUpdate({ expectSignatureKeyId: 'unknown' }),
    WebviewBundleError
  );
  assertEquals(error.code, 'core.expect_signature_not_found');
});

Deno.test('Updater fails closed on an invalid signature key', async () => {
  await using server = startUpdateServer();
  const bad = (key: SignatureVerifyKey) => () => setup(server, { signature: { keys: [key] } });

  // A key too short to be an ed25519 public key: the updater must not fall back to unverified.
  const short = assertThrows(
    bad({
      id: 'k',
      verify: { algorithm: 'ed25519', key: { format: 'raw', data: new Uint8Array(4) } },
    }),
    WebviewBundleError
  );
  assertEquals(short.code, 'invalid_signature_key');

  // A PEM format given bytes (or the reverse) is a caller mistake, not an unverified fallback.
  const wrongType = assertThrows(
    bad({
      id: 'k',
      verify: { algorithm: 'ed25519', key: { format: 'spki_pem', data: new Uint8Array(4) } },
    }),
    WebviewBundleError
  );
  assertEquals(wrongType.code, 'invalid_signature_key');

  const notAKey = assertThrows(
    bad({
      id: 'k',
      verify: { algorithm: 'ed25519', key: { format: 'spki_pem', data: 'not a valid key' } },
    }),
    WebviewBundleError
  );
  assertEquals(notAKey.code, 'invalid_signature_key');
});

Deno.test('Updater fails closed on an unknown integrity policy', async () => {
  await using server = startUpdateServer();
  const error = assertThrows(
    // 'none' was the old spelling of 'off'; it must fail construction rather than be ignored.
    () => setup(server, { integrity: { policy: 'none' } } as unknown as UpdaterOptions),
    WebviewBundleError
  );
  assertEquals(error.code, 'invalid_request');
});
