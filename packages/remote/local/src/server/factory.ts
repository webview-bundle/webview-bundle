import { Readable } from 'node:stream';
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

export type WebviewBundleServer = Hono<Env>;

export interface WebviewBundleServerConfig extends Omit<Config<Env>, 'getUpdate' | 'download'> {
  /**
   * @default "~/.wvb/local"
   */
  baseDir?: string;
}

export function buildWebviewBundleServer(
  config: WebviewBundleServerConfig = {}
): WebviewBundleServer {
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

  return server;
}

export const buildWvbServer = buildWebviewBundleServer;
