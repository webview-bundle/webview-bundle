import { type WorkerConfig, webviewBundleWorker } from '../src/worker/index.js';

declare const __WVB_REMOTE_CONFIG__: Pick<WorkerConfig, 'signaturePolicy' | 'disableIfNoneMatch'>;
const worker = webviewBundleWorker(__WVB_REMOTE_CONFIG__);

export default worker;
