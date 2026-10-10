import { type WebviewBundleRemoteConfig, wvbRemoteHandler } from '../src/lambda/index.js';

declare const __WVB_REMOTE_CONFIG__: WebviewBundleRemoteConfig;
export const handler = wvbRemoteHandler(__WVB_REMOTE_CONFIG__);
