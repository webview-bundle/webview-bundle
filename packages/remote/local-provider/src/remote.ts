import { createHash } from 'node:crypto';
import os from 'node:os';
import path from 'node:path';
import { Readable } from 'node:stream';
import {
  getBundleFileSize,
  readBundleStream,
  readUpdateFile,
  stringifyUpdate,
} from '@wvb/remote-local/api';
import { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { parseExpectSignatureHeader } from './signature.js';

const UPDATE_PROTOCOL_VERSION = '1';

interface Env {
  // biome-ignore lint/complexity/noBannedTypes: expected
  Bindings: {};
}

export type WebviewBundleRemote = Hono<Env>;

export interface WebviewBundleRemoteConfig {
  /**
   * Base directory for bundle storage.
   * @default `~/.wvb/local`
   */
  baseDir?: string;
  /**
   * @default 'optional'
   */
  signaturePolicy?: 'strict' | 'optional' | 'off';
}

export function webviewBundleRemote({
  baseDir: inputBaseDir,
  signaturePolicy = 'optional',
}: WebviewBundleRemoteConfig) {
  const app = new Hono<Env>();
  const baseDir = inputBaseDir ?? path.join(os.homedir(), '.wvb', 'local');

  app.onError((error, c) => {
    console.error(error);
    return c.json({ message: error.message }, 500);
  });

  app.get('/update', async c => {
    const protocolVersion = c.req.header('wvb-update-protocol-version');
    if (protocolVersion !== UPDATE_PROTOCOL_VERSION) {
      return c.json(
        { message: `Unsupported update protocol version: ${protocolVersion ?? '(missing)'}` },
        400
      );
    }

    const channel = c.req.header('wvb-update-channel');
    const update = await readUpdateFile({ baseDir, channel });
    if (update == null) {
      return c.body(null, 204);
    }

    const expectSignature = parseExpectSignatureHeader(c.req.header('wvb-expect-signature'));
    const signature = update.signatures.find(
      x => x.id === expectSignature?.keyId && x.alg === expectSignature?.alg
    );
    switch (signaturePolicy) {
      case 'strict': {
        if (signature == null) {
          return c.json({ message: 'Missing expect signature' }, 400);
        }
        break;
      }
      case 'optional': {
        if (expectSignature != null && signature == null) {
          return c.json({ message: 'Missing expect signature' }, 400);
        }
        break;
      }
    }

    const body = stringifyUpdate(update.update);
    const etag = `"${createHash('sha256').update(body).digest('hex')}"`;

    c.header('etag', etag);
    if (c.req.header('if-none-match') === etag) {
      return c.body(null, 304);
    }

    c.header('content-type', 'application/json; charset=UTF-8');
    if (signature != null) {
      c.header(
        'wvb-signature',
        `key_id="${signature.id}", alg="${signature.alg}", sig="${signature.sig}"`
      );
    }

    return c.body(body, 200);
  });

  app.get('/bundles/:name/:version', async c => {
    const name = c.req.param('name');
    const version = c.req.param('version');
    const size = await getBundleFileSize({ baseDir, bundle: name, version });

    c.header('content-length', String(size));
    c.header('content-type', 'application/webview-bundle');

    if (c.req.method.toUpperCase() === 'HEAD') {
      return c.body(null);
    }

    const bundleStream = await readBundleStream({ baseDir, bundle: name, version });
    if (bundleStream == null) {
      return c.body(null, 404);
    }

    return stream(c, async output => {
      await output.pipe(Readable.toWeb(bundleStream) as ReadableStream);
    });
  });

  return app;
}

export const wvbRemote = webviewBundleRemote;
