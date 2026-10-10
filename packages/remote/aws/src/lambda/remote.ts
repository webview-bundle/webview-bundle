import { S3Client } from '@aws-sdk/client-s3';
import { buildServer as baseRemote, type Config } from '@wvb/remote-base/server';
import type { Hono } from 'hono';
import type { Bindings, Context } from './types.js';
import { getUpdate } from './update.js';

export interface Env {
  Bindings: Bindings;
}

export interface WebviewBundleRemoteConfig extends Omit<Config<Env>, 'getUpdate' | 'download'> {
  bucketName: string;
  region?: string;
  s3Client?: S3Client;
}

export type WebviewBundleRemote = Hono<Env>;

export function webviewBundleRemote(config: WebviewBundleRemoteConfig): WebviewBundleRemote {
  const { bucketName, region, s3Client = new S3Client({ region }), ...remoteConfig } = config;
  const context: Context = {
    bucketName,
    s3Client,
  };

  const remote = baseRemote({
    ...remoteConfig,
    getUpdate: async params => {
      const update = await getUpdate(context, {
        channel: params.channel,
        runtimeVersion: params.runtimeVersion,
      });
      return update;
    },
    download: async params => {
      const request = params.context.env.request;
      request.uri = `/bundles/${params.bundleName}/${params.version}.wvb`;

      params.context.env.callback(null, request);
      return params.context.body(null);
    },
  });

  return remote;
}

export const wvbRemote = webviewBundleRemote;
