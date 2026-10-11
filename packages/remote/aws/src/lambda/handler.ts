import type { CloudFrontRequestEvent, CloudFrontRequestResult, Handler } from 'aws-lambda';
import { handle } from 'hono/lambda-edge';
import { type WebviewBundleRemoteConfig, wvbRemote } from './remote.js';

export type WebviewBundleRemoteHandler = Handler<CloudFrontRequestEvent, CloudFrontRequestResult>;

export function webviewBundleRemoteHandler(
  config: WebviewBundleRemoteConfig
): WebviewBundleRemoteHandler {
  return handle(wvbRemote(config)) as WebviewBundleRemoteHandler;
}
