import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';
import { serve } from '@hono/node-server';
import getPort from 'get-port';
import { Hono } from 'hono';
import {
  BundleBuilder,
  RUNTIME_VERSION,
  UPDATE_PROTOCOL_VERSION,
  writeBundleIntoBuffer,
} from '../dist/index.js';

export interface ServedBundle {
  name: string;
  version: string;
  data: Buffer;
  integrity?: string;
  downloadUrl?: string;
  metadata?: Record<string, string>;
}

export type UpdateSigner = (params: {
  body: Buffer;
  keyId: string;
  alg: string;
}) => Promise<string> | string;

interface UpdateServerState {
  bundles: ServedBundle[];
  metadata: Record<string, string>;
  createdAt: string;
  signer: UpdateSigner | undefined;
  failWith: { status: number; message?: string } | undefined;
  lastRequest: { url: string; headers: Headers } | undefined;
}

export interface UpdateServer extends UpdateServerState {
  readonly baseUrl: string;
  close(): Promise<void>;
}

export function buildBundleData(name: string, version: string): Buffer {
  const builder = new BundleBuilder();
  builder.insertEntry('/index.html', Buffer.from(`<h1>${name}@${version}</h1>`, 'utf8'));
  builder.insertEntry('/app.js', Buffer.from(`console.log("${name}");`, 'utf8'));
  return Buffer.from(writeBundleIntoBuffer(builder.build()));
}

export async function startUpdateServer(): Promise<UpdateServer> {
  const port = await getPort();
  const state: UpdateServerState = {
    bundles: [],
    metadata: {},
    createdAt: '2026-01-01T00:00:00Z',
    signer: undefined,
    failWith: undefined,
    lastRequest: undefined,
  };

  const app = new Hono();

  app.get('/update', async c => {
    state.lastRequest = { url: c.req.url, headers: c.req.raw.headers };
    if (state.failWith != null) {
      return json({ message: state.failWith.message }, state.failWith.status);
    }
    if (c.req.header('wvb-update-protocol-version') !== UPDATE_PROTOCOL_VERSION) {
      return json({ message: 'unsupported update protocol version' }, 400);
    }

    const channel = c.req.header('wvb-update-channel');
    const body = Buffer.from(
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
      }),
      'utf8'
    );
    const etag = `"${createHash('sha256').update(body).digest('hex').slice(0, 32)}"`;
    if (c.req.header('if-none-match') === etag) {
      return new Response(null, { status: 304, headers: { etag } });
    }

    const headers = new Headers({ 'content-type': 'application/json', etag });
    const expect = c.req.header('wvb-expect-signature');
    if (expect != null && state.signer != null) {
      const keyId = dictValue(expect, 'key_id') ?? '';
      const alg = dictValue(expect, 'alg') ?? '';
      const sig = await state.signer({ body, keyId, alg });
      headers.set('wvb-signature', `key_id="${keyId}", alg="${alg}", sig="${sig}"`);
    }
    return new Response(new Uint8Array(body), { status: 200, headers });
  });

  app.get('/bundles/:name/:version', c => {
    const bundle = state.bundles.find(
      x => x.name === c.req.param('name') && x.version === c.req.param('version')
    );
    if (bundle == null) {
      return json({ message: 'bundle not found' }, 404);
    }
    return new Response(new Uint8Array(bundle.data), {
      status: 200,
      headers: { 'content-type': 'application/webview-bundle' },
    });
  });

  const server = serve({ fetch: app.fetch, port });

  return Object.assign(state, {
    baseUrl: `http://localhost:${port}`,
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close(e => (e != null ? reject(e) : resolve()));
      }),
  });
}

function json(body: unknown, status: number): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function updateId(bundles: ServedBundle[], channel: string | undefined): string {
  const seed = [channel ?? '', ...bundles.map(x => `${x.name}@${x.version}`)].join('\0');
  return createHash('sha256').update(seed).digest('hex').slice(0, 32);
}

function dictValue(header: string, key: string): string | undefined {
  return new RegExp(`${key}="([^"]*)"`).exec(header)?.[1];
}
