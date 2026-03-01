import type { RemoteBundleInfo } from '@wvb/node';
import type { Logger } from '../../log.js';
import { c } from '../../console.js';
import { formatByteLength } from '../../format.js';

export function logRemoteBundleInfo(
  logger: Logger,
  remoteBundle: RemoteBundleInfo,
  byteLength: number
): void {
  const { name: bundleName, version, etag, integrity, signature, lastModified } = remoteBundle;
  logger.info(
    `Remote Webview Bundle: ${c.info(bundleName)} ${c.bytes(formatByteLength(byteLength))}`
  );
  logger.info(`  Version: ${c.bold(c.info(version))}`);
  logger.info(`  ETag: ${c.bold(c.info(etag ?? '(none)'))}`);
  logger.info(`  Integrity: ${c.bold(c.info(integrity ?? '(none)'))}`);
  logger.info(`  Signature: ${c.bold(c.info(signature ?? '(none)'))}`);
  logger.info(`  Last-Modified: ${c.bold(c.info(lastModified ?? '(none)'))}`);
}
