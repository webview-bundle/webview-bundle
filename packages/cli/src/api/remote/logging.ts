import type { BundleUpdate } from '@wvb/node';
import { c } from '../../console.js';
import { formatByteLength } from '../../format.js';
import type { Logger } from '../../log.js';

export function logRemoteBundleInfo(
  logger: Logger,
  remoteBundle: BundleUpdate,
  byteLength: number
): void {
  const { name: bundleName, version, integrity } = remoteBundle;
  logger.info(
    `Remote Webview Bundle: ${c.info(bundleName)} ${c.bytes(formatByteLength(byteLength))}`
  );
  logger.info(`  Version: ${c.bold(c.info(version))}`);
  logger.info(`  Integrity: ${c.bold(c.info(integrity ?? '(none)'))}`);
  logger.info(`  Download URL: ${c.bold(c.info(remoteBundle.downloadUrl ?? '(default)'))}`);
}
