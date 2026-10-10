import path from 'node:path';
import { type BaseUploader, type IntegrityMakeConfig, makeIntegrity } from '@wvb/config/remote';
import { type Bundle, readBundle, writeBundleIntoBuffer } from '@wvb/node';
import { c } from '../console.js';
import { formatByteLength } from '../format.js';
import { pathExists, toAbsolutePath } from '../fs.js';
import type { Logger } from '../log.js';
import { ApiError } from './error.js';

export interface RemoteUploadParams {
  file: string | Bundle;
  bundleName: string;
  version: string;
  uploader: BaseUploader;
  integrity?: boolean | IntegrityMakeConfig;
  metadata?: Record<string, string>;
  logger?: Logger;
  cwd?: string;
}

/**
 * Upload Webview Bundle to remote server.
 */
export async function remoteUpload(params: RemoteUploadParams): Promise<void> {
  const {
    file,
    bundleName: bundleNameInput,
    version,
    uploader,
    integrity: integrityConfig = true,
    metadata,
    logger,
    cwd = process.cwd(),
  } = params;

  let bundle: Bundle;
  if (typeof file === 'string') {
    const filepath = toAbsolutePath(file, cwd);
    if (!(await pathExists(filepath))) {
      const message = `File does not exist: ${filepath}`;
      logger?.error(message);
      throw new ApiError(message);
    }
    bundle = await readBundle(filepath);
  } else {
    bundle = file;
  }
  const bundleName =
    bundleNameInput ?? (typeof file === 'string' ? path.basename(file, '.wvb') : undefined);
  if (bundleName == null) {
    const message = `Cannot get bundle name. If you pass "file" as bundle object, you must provide "bundleName" field.`;
    logger?.error(message);
    throw new ApiError(message);
  }
  logger?.info(
    `Will upload Remote Webview Bundle: ${c.bold(c.info(bundleName))} (Version: ${c.info(version)})`
  );

  const buf = writeBundleIntoBuffer(bundle);
  const size = buf.byteLength;

  let integrity: string | undefined;
  if (integrityConfig !== false) {
    const opts = typeof integrityConfig === 'boolean' ? {} : integrityConfig;
    integrity = await makeIntegrity(opts, buf);
    logger?.info(`Integrity: ${integrity}`);
  } else {
    logger?.info('Skip integrity making.');
  }

  await uploader.upload({
    bundle: buf,
    name: bundleName,
    version,
    versionData: {
      integrity,
      metadata,
    },
  });
  logger?.info(`Webview Bundle uploaded: ${c.info(bundleName)} ${c.bytes(formatByteLength(size))}`);
  logger?.info(`  Version: ${c.bold(c.info(version))}`);
  logger?.info(`  Integrity: ${c.bold(c.info(integrity ?? '(none)'))}`);
}
