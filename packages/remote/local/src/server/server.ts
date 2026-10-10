import type { AddressInfo } from 'node:net';
import { Readable } from 'node:stream';
import { type ServerType, serve } from '@hono/node-server';
import type { Config } from '@wvb/remote-base/server';
import { buildServer } from '@wvb/remote-base/server';
import type { Hono } from 'hono';
import { stream } from 'hono/streaming';
import { getBundleFileSize, readBundleStream, readCurrentUpdate } from '../api/index.js';
import { getDefaultBaseDir } from '../base-dir.js';

interface Env {
  // biome-ignore lint/complexity/noBannedTypes: expected
  Bindings: {};
}

export interface WebviewBundleServerInstance {
  server: ServerType;
  shutdown(): Promise<void>;
}

export interface WebviewBundleServerServeParams extends Omit<Parameters<typeof serve>[0], 'fetch'> {
  onListen?: (info: AddressInfo) => void;
}

export type WebviewBundleServer = Hono<Env> & {
  serve(params: WebviewBundleServerServeParams): WebviewBundleServerInstance;
};

export interface WebviewBundleServerConfig extends Omit<Config<Env>, 'getUpdate' | 'download'> {
  /**
   * @default "~/.wvb/local"
   */
  baseDir?: string;
}

export function webviewBundleServer(config: WebviewBundleServerConfig = {}): WebviewBundleServer {
  const { baseDir = getDefaultBaseDir(), ...serverOptions } = config;
  const server = buildServer({
    ...serverOptions,
    getUpdate: async ({ runtimeVersion, channel }) => {
      const update = await readCurrentUpdate({ baseDir, runtimeVersion, channel });
      return update;
    },
    download: async ({ context, bundleName, version }) => {
      const size = await getBundleFileSize({ baseDir, bundle: bundleName, version });

      if (size != null) {
        context.header('content-length', String(size));
      }
      context.header('content-type', 'application/webview-bundle');

      if (context.req.method.toUpperCase() === 'HEAD') {
        return context.body(null);
      }

      const bundleStream = await readBundleStream({ baseDir, bundle: bundleName, version });
      if (bundleStream == null) {
        return context.body(null, 404);
      }

      return stream(context, async output => {
        await output.pipe(Readable.toWeb(bundleStream) as ReadableStream);
      });
    },
  });

  Object.assign(server, {
    serve: ({ onListen, ...options }: WebviewBundleServerServeParams) => {
      const nodeServer = serve(
        {
          ...(options as any),
          fetch: server.fetch,
        },
        onListen
      );
      const instance: WebviewBundleServerInstance = {
        server: nodeServer,
        shutdown(): Promise<void> {
          return new Promise<void>((resolve, reject) => {
            nodeServer.close(error => {
              if (error != null) {
                reject(error);
              } else {
                resolve();
              }
            });
          });
        },
      };
      return instance;
    },
  });

  return server as WebviewBundleServer;
}
