import type { KVNamespace, R2Bucket } from '@cloudflare/workers-types';
import type { Current, Update } from '@wvb/remote-base';
import { buildServer, type Config } from '@wvb/remote-base/server';
import type { Hono } from 'hono';

export interface WorkerBindings {
  KV: KVNamespace;
  BUCKET: R2Bucket;
}

export interface WorkerEnv {
  Bindings: WorkerBindings;
}

export type WorkerConfig = Omit<Config<WorkerEnv>, 'getUpdate' | 'download'>;
export type WebviewBundleWorker = Hono<WorkerEnv>;

export function webviewBundleWorker(config: WorkerConfig = {}): WebviewBundleWorker {
  return buildServer<WorkerEnv>({
    ...config,
    async getUpdate({ context, runtimeVersion, channel }) {
      const prefix = `${channel == null ? '' : `channels/${channel}/`}updates/${runtimeVersion}`;

      const current = await context.env.KV.get<Current>(`${prefix}/current.json`, 'json');
      if (current == null) {
        return null;
      }

      const update = await context.env.KV.get<Update>(`${prefix}/${current.updateId}.json`, 'json');
      return update;
    },
    async download({ context, bundleName, version }) {
      const object = await context.env.BUCKET.get(`bundles/${bundleName}/${version}.wvb`);
      if (object == null) {
        return context.body(null, 404);
      }

      const headers = new Headers();
      object.writeHttpMetadata(headers);

      headers.set('content-type', 'application/webview-bundle');
      headers.set('content-length', String(object.size));
      headers.set('etag', object.httpEtag);

      if (context.req.method === 'HEAD') {
        return new Response(null, { headers });
      }

      return new Response(object.body, { headers });
    },
  });
}
