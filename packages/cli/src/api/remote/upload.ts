import {
  type BaseRemoteUploader,
  type IntegrityMakeConfig,
  makeIntegrity,
  type SignatureSignConfig,
  signSignature,
} from '@wvb/config/remote';
import { type Bundle, readBundle, writeBundleIntoBuffer } from '@wvb/node';
import { Buffer } from 'node:buffer';
import path from 'node:path';
import type { Logger } from '../../log.js';
import { c } from '../../console.js';
import { formatByteLength } from '../../format.js';
import { pathExists, toAbsolutePath } from '../../fs.js';
import { ApiError } from '../error.js';

export interface RemoteUploadParams {
  file: string | Bundle;
  bundleName?: string;
  version: string;
  uploader: BaseRemoteUploader;
  force?: boolean;
  integrity?: boolean | IntegrityMakeConfig;
  signature?: SignatureSignConfig;
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
    force,
    integrity: integrityConfig,
    signature: signatureConfig,
    logger,
    cwd,
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
    `Will upload Remote Webview Bundle: ${c.bold(c.info(bundleName))} (Version: ${c.info(version)}`
  );

  const buf = writeBundleIntoBuffer(bundle);
  const size = buf.byteLength;

  let integrity: string | undefined;
  if (integrityConfig != null && integrityConfig !== false) {
    integrity = await makeIntegrity(integrityConfig === true ? {} : integrityConfig, buf);
    logger?.info(`Integrity: ${integrity}`);
  } else {
    logger?.info('Skip integrity making.');
  }

  let signature: string | undefined;
  if (signatureConfig != null) {
    if (integrity == null) {
      const message =
        'Cannot make signature without integrity. Make sure integrity making option is enabled.';
      logger?.error(message);
      throw new ApiError(message);
    }
    signature = await signSignature(signatureConfig, Buffer.from(integrity, 'utf8'));
    logger?.info(`Signature: ${signature}`);
  } else {
    logger?.info('Skip signature signing.');
  }
  await uploader.upload({
    bundle: buf,
    bundleName,
    version,
    force,
    integrity,
    signature,
  });
  logger?.info(`Webview Bundle uploaded: ${c.info(bundleName)} ${c.bytes(formatByteLength(size))}`);
  logger?.info(`  Version: ${c.bold(c.info(version))}`);
  logger?.info(`  Integrity: ${c.bold(c.info(integrity ?? '(none)'))}`);
  logger?.info(`  Signature: ${c.bold(c.info(signature ?? '(none)'))}`);
}
