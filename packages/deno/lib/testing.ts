// Helpers shared by the `*.test.ts` files. Excluded from the published package (see `deno.json`).
import { encodeBase64 } from '@std/encoding/base64';
import { fromFileUrl } from '@std/path';
import { BundleBuilder, writeBundleToBytes } from './bundle.ts';
import { RUNTIME_VERSION, UPDATE_PROTOCOL_VERSION } from './consts.ts';
import { loadLib } from './ffi.ts';
import { Source, type SourceConfig, type SourceOptions } from './source.ts';

const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';

/** The locally-built cdylib (`cargo build --release -p wvb-deno`). */
export const DYLIB: string = fromFileUrl(
  new URL(`../../../target/release/${prefix}wvb_deno.${ext}`, import.meta.url)
);

/** The committed builtin fixture: bundle "app" v1.0.0. */
export const BUILTIN_DIR: string = fromFileUrl(new URL('../fixtures/builtin', import.meta.url));

/**
 * Load the native library. Call it at module top level — not inside a test — so Deno's per-test
 * resource-leak sanitizer doesn't flag the intentionally process-lifetime FFI handle.
 */
export function loadTestLib(): void {
  loadLib(DYLIB);
}

/** A temp directory removed when the returned disposable goes out of scope. */
export function tempDir(prefix: string): { path: string } & Disposable {
  const path = Deno.makeTempDirSync({ prefix });
  return {
    path,
    [Symbol.dispose]: () => {
      try {
        Deno.removeSync(path, { recursive: true });
      } catch {
        // Already gone: nothing to clean up.
      }
    },
  };
}

/**
 * A {@link Source} over the builtin fixture, backed by a temp remote dir. Disposing it frees the
 * handle and removes the temp dir, so tests only need `using source = testSource()`.
 */
export function testSource(options?: SourceOptions, config?: Partial<SourceConfig>): Source {
  const remoteDir = Deno.makeTempDirSync({ prefix: 'wvb-deno-test-' });
  const removeRemoteDir = () => Deno.removeSync(remoteDir, { recursive: true });
  try {
    const source = new Source({ builtinDir: BUILTIN_DIR, remoteDir, options, ...config });
    return Object.assign(source, {
      [Symbol.dispose]: () => {
        try {
          source.free();
        } finally {
          removeRemoteDir();
        }
      },
    });
  } catch (e) {
    // The source never took ownership of the temp dir, so nothing else will remove it.
    removeRemoteDir();
    throw e;
  }
}

/** A `.wvb` file's bytes, carrying `/index.html` and `/app.js`. */
export function buildBundleData(name: string, version: string): Uint8Array<ArrayBuffer> {
  using builder = new BundleBuilder();
  const encoder = new TextEncoder();
  builder.insertEntry('/index.html', encoder.encode(`<h1>${name}@${version}</h1>`));
  builder.insertEntry('/app.js', encoder.encode(`console.log("${name}");`));
  using bundle = builder.build();
  return writeBundleToBytes(bundle);
}

export interface ServedBundle {
  name: string;
  version: string;
  data: Uint8Array<ArrayBuffer>;
  integrity?: string;
  downloadUrl?: string;
  metadata?: Record<string, string>;
}

/** Signs the update body the server is about to send, mirroring a real signing service. */
export type UpdateSigner = (params: {
  body: Uint8Array<ArrayBuffer>;
  keyId: string;
  alg: string;
}) => Promise<string> | string;

export interface UpdateServer extends AsyncDisposable {
  readonly baseUrl: string;
  bundles: ServedBundle[];
  metadata: Record<string, string>;
  createdAt: string;
  signer?: UpdateSigner;
  failWith?: { status: number; message?: string };
  lastRequest?: { url: string; headers: Headers };
  close(): Promise<void>;
}

/** A stand-in for the update server the remote talks to (`GET /update`, `GET /bundles/:n/:v`). */
export function startUpdateServer(): UpdateServer {
  const state = {
    bundles: [] as ServedBundle[],
    metadata: {} as Record<string, string>,
    createdAt: '2026-01-01T00:00:00Z',
    signer: undefined as UpdateSigner | undefined,
    failWith: undefined as { status: number; message?: string } | undefined,
    lastRequest: undefined as { url: string; headers: Headers } | undefined,
  };

  const server = Deno.serve({ hostname: '127.0.0.1', port: 0, onListen: () => {} }, async req => {
    const { pathname } = new URL(req.url);
    if (pathname === '/update') {
      return await serveUpdate(state, req);
    }
    const matched = /^\/bundles\/([^/]+)\/([^/]+)$/.exec(pathname);
    if (matched != null) {
      const bundle = state.bundles.find(x => x.name === matched[1] && x.version === matched[2]);
      if (bundle == null) {
        return json({ message: 'bundle not found' }, 404);
      }
      return new Response(bundle.data, {
        status: 200,
        headers: { 'content-type': 'application/webview-bundle' },
      });
    }
    return json({ message: 'not found' }, 404);
  });

  const close = async () => {
    await server.shutdown();
  };
  return Object.assign(state, {
    baseUrl: `http://127.0.0.1:${server.addr.port}`,
    close,
    [Symbol.asyncDispose]: close,
  });
}

interface ServerState {
  bundles: ServedBundle[];
  metadata: Record<string, string>;
  createdAt: string;
  signer?: UpdateSigner;
  failWith?: { status: number; message?: string };
  lastRequest?: { url: string; headers: Headers };
}

async function serveUpdate(state: ServerState, req: Request): Promise<Response> {
  state.lastRequest = { url: req.url, headers: req.headers };
  if (state.failWith != null) {
    return json({ message: state.failWith.message }, state.failWith.status);
  }
  if (req.headers.get('wvb-update-protocol-version') !== UPDATE_PROTOCOL_VERSION) {
    return json({ message: 'unsupported update protocol version' }, 400);
  }

  const channel = req.headers.get('wvb-update-channel') ?? undefined;
  const body = new TextEncoder().encode(
    JSON.stringify({
      id: updateId(state.bundles, channel),
      createdAt: state.createdAt,
      runtimeVersion: RUNTIME_VERSION,
      bundles: state.bundles.map(bundle => ({
        name: bundle.name,
        version: bundle.version,
        downloadUrl: bundle.downloadUrl,
        integrity: bundle.integrity,
        metadata: bundle.metadata,
      })),
      metadata: channel != null ? { ...state.metadata, channel } : state.metadata,
    })
  );
  const etag = `"${await sha256Hex(body)}"`.slice(0, 34);
  if (req.headers.get('if-none-match') === etag) {
    return new Response(null, { status: 304, headers: { etag } });
  }

  const headers = new Headers({ 'content-type': 'application/json', etag });
  const expect = req.headers.get('wvb-expect-signature');
  if (expect != null && state.signer != null) {
    const keyId = dictValue(expect, 'key_id') ?? '';
    const alg = dictValue(expect, 'alg') ?? '';
    const sig = await state.signer({ body, keyId, alg });
    headers.set('wvb-signature', `key_id="${keyId}", alg="${alg}", sig="${sig}"`);
  }
  return new Response(body, { status: 200, headers });
}

function json(body: unknown, status: number): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

/** The update id changes with what is served, so a changed catalog is a changed update. */
function updateId(bundles: ServedBundle[], channel: string | undefined): string {
  return [channel ?? '', ...bundles.map(x => `${x.name}@${x.version}`)].join('|');
}

function dictValue(header: string, key: string): string | undefined {
  return new RegExp(`${key}="([^"]*)"`).exec(header)?.[1];
}

async function sha256Hex(data: Uint8Array<ArrayBuffer>): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', data));
  return Array.from(digest, byte => byte.toString(16).padStart(2, '0')).join('');
}

/** An ed25519 key pair plus a signer over the raw update body, for the signature tests. */
export async function ed25519Signer(): Promise<{
  publicKeyDer: Uint8Array<ArrayBuffer>;
  sign: UpdateSigner;
}> {
  const keyPair = (await crypto.subtle.generateKey({ name: 'Ed25519' }, true, [
    'sign',
    'verify',
  ])) as CryptoKeyPair;
  const publicKeyDer = new Uint8Array(await crypto.subtle.exportKey('spki', keyPair.publicKey));
  return {
    publicKeyDer,
    sign: async ({ body }) =>
      encodeBase64(new Uint8Array(await crypto.subtle.sign('Ed25519', keyPair.privateKey, body))),
  };
}
