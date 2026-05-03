import { Hono } from 'hono';
import type { Context } from './context.js';
import { getBundleDataResponse } from './operations/getBundleDataResponse.js';
import { getBundleVersion } from './operations/getBundleVersion.js';
import { listAllBundleDeployments } from './operations/listAllBundleDeployments.js';
import { getRemoteBundleDeploymentVersion } from './types.js';

export type WebviewBundleRemote = Hono<{ Bindings: Context }>;

export interface WebviewBundleRemoteOptions {
  /** Option to allow downloading other version instead of deployed version */
  allowOtherVersions?: boolean;
}

export function webviewBundleRemote(options: WebviewBundleRemoteOptions = {}): WebviewBundleRemote {
  const { allowOtherVersions = false } = options;

  const app = new Hono<{ Bindings: Context }>();

  app.get('/bundles', async c => {
    const channel = c.req.query('channel');
    const deployments = await listAllBundleDeployments(c.env);
    const bundles = deployments
      .map(deployment => {
        const version = getRemoteBundleDeploymentVersion(deployment, channel);
        if (version == null) {
          return null;
        }
        return { name: deployment.name, version };
      })
      .filter(x => x != null);
    return c.json(bundles);
  });

  app.get('/bundles/:name', async c => {
    const bundleName = c.req.param('name');
    const channel = c.req.query('channel');
    const version = await getBundleVersion(c.env, bundleName, channel);
    if (version == null) {
      return c.notFound();
    }
    const resp = await getBundleDataResponse(c.env, bundleName, version);
    if (resp == null) {
      return c.notFound();
    }
    return resp;
  });

  app.get('/bundles/:name/:version', async c => {
    if (!allowOtherVersions) {
      return new Response(null, { status: 403 });
    }
    const bundleName = c.req.param('name');
    const version = c.req.param('version');
    const resp = await getBundleDataResponse(c.env, bundleName, version);
    if (resp == null) {
      return c.notFound();
    }
    return resp;
  });

  return app;
}

export const wvbRemote = webviewBundleRemote;
