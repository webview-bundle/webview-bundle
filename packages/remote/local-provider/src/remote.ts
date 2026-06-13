import os from 'node:os';
import path from 'node:path';
import { Readable } from 'node:stream';
import {
  bundleFileSize,
  readAllDeployments,
  readBundleMetadata,
  readBundleStream,
  readDeployment,
} from '@wvb/remote-local/api';
import { type Context, Hono } from 'hono';
import { stream } from 'hono/streaming';
import type { Variables } from './types.js';
import { getRemoteBundleDeploymentVersion } from './utils.js';

interface Env {
  // biome-ignore lint/complexity/noBannedTypes: expected
  Bindings: {};
  Variables: Variables;
}

export type WebviewBundleRemote = Hono<Env>;

export interface WebviewBundleRemoteConfig {
  /**
   * Base directory for bundle storage.
   * @default `~/.wvb/local`
   */
  baseDir?: string;
  /** Option to allow downloading other version instead of deployed version */
  allowOtherVersions?: boolean;
}

export function webviewBundleRemote({ baseDir, allowOtherVersions }: WebviewBundleRemoteConfig) {
  const app = new Hono<Env>();

  app.use(async (c, next) => {
    c.set('baseDir', baseDir ?? path.join(os.homedir(), '.wvb', 'local'));
    await next();
  });

  app.get('/bundles', async c => {
    const channel = c.req.query('channel');
    const deployments = await readAllDeployments({ baseDir: c.get('baseDir') });
    const bundles = deployments
      .map(x => {
        const version = getRemoteBundleDeploymentVersion(x, channel);
        if (version == null) {
          return null;
        }
        return { name: x.name, version };
      })
      .filter(x => x != null);
    return c.json(bundles);
  });

  async function getBundleResponse(c: Context<Env>, bundle: string, version: string) {
    const metadata = await readBundleMetadata({
      baseDir: c.get('baseDir'),
      bundle,
      version,
    });
    c.header('webview-bundle-name', bundle);
    c.header('webview-bundle-version', version);
    if (metadata?.integrity != null) {
      c.header('webview-bundle-integrity', metadata.integrity);
    }
    if (metadata?.signature != null) {
      c.header('webview-bundle-signature', metadata.signature);
    }

    const size = await bundleFileSize({ baseDir: c.get('baseDir'), bundle, version });
    c.header('content-length', String(size));

    if (c.req.method.toUpperCase() === 'HEAD') {
      return c.body(null);
    }

    return stream(c, async s => {
      const bundleStream = readBundleStream({
        baseDir: c.get('baseDir'),
        bundle,
        version,
      });
      await s.pipe(Readable.toWeb(bundleStream) as ReadableStream);
    });
  }

  app.get('/bundles/:name', async c => {
    const bundle = c.req.param('name');
    const channel = c.req.query('channel');
    const deployment = await readDeployment({
      bundle,
      baseDir: c.get('baseDir'),
    });
    if (deployment == null) {
      return c.notFound();
    }
    const version = getRemoteBundleDeploymentVersion(deployment, channel, true);
    if (version == null) {
      return c.notFound();
    }
    return await getBundleResponse(c, bundle, version);
  });

  app.get('/bundles/:name/:version', async c => {
    if (allowOtherVersions !== true) {
      return c.body(null, { status: 403 });
    }
    const bundle = c.req.param('name');
    const version = c.req.param('version');
    return await getBundleResponse(c, bundle, version);
  });

  return app;
}

export const wvbRemote = webviewBundleRemote;
