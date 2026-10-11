import type { Buffer } from 'node:buffer';
import { createReadStream, type ReadStream } from 'node:fs';
import fs from 'node:fs/promises';
import path from 'node:path';
import writeFileAtomic from 'write-file-atomic';
import { z } from 'zod';
import { lock } from './lock.js';
import { isFileNotFoundError, normalizeBundleName } from './utils.js';

interface ReadBundleStreamParams {
  baseDir: string;
  bundle: string;
  version: string;
}

export async function readBundleStream({
  baseDir,
  bundle,
  version,
}: ReadBundleStreamParams): Promise<ReadStream | null> {
  const filePath = getBundleFilePath(baseDir, bundle, version);
  try {
    await fs.access(filePath);
  } catch (e) {
    if (isFileNotFoundError(e)) {
      return null;
    }
    throw e;
  }
  return createReadStream(filePath);
}

export const BundleVersionDataSchema = z.object({
  integrity: z.string().optional(),
  metadata: z.record(z.string(), z.string()).optional(),
});
export type BundleVersionData = z.infer<typeof BundleVersionDataSchema>;

interface ReadBundleVersionDataParams {
  baseDir: string;
  bundle: string;
  version: string;
}

export async function readBundleVersionData({
  baseDir,
  bundle,
  version,
}: ReadBundleVersionDataParams): Promise<BundleVersionData | null> {
  try {
    const filePath = getBundleVersionDataFilePath(baseDir, bundle, version);
    const raw = await fs.readFile(filePath, 'utf8');

    return BundleVersionDataSchema.parse(JSON.parse(raw));
  } catch (e) {
    if (isFileNotFoundError(e)) {
      return null;
    }
    throw e;
  }
}

export async function getBundleFileSize({
  baseDir,
  bundle,
  version,
}: ReadBundleStreamParams): Promise<number | null> {
  try {
    const filePath = getBundleFilePath(baseDir, bundle, version);
    const stats = await fs.stat(filePath);

    return stats.size;
  } catch (e) {
    if (isFileNotFoundError(e)) {
      return null;
    }
    throw e;
  }
}

interface WriteBundleParams {
  baseDir: string;
  bundle: string;
  version: string;
  data: Buffer;
}

export async function writeBundle({
  baseDir,
  bundle,
  version,
  data,
}: WriteBundleParams): Promise<void> {
  await lock.acquire();
  try {
    const filePath = getBundleFilePath(baseDir, bundle, version);
    await fs.mkdir(path.dirname(filePath), { recursive: true });
    await writeFileAtomic(filePath, data);
  } finally {
    lock.release();
  }
}

interface WriteBundleVersionDataParams {
  baseDir: string;
  bundle: string;
  version: string;
  data: {
    integrity?: string;
    metadata?: Record<string, string>;
  };
}

export async function writeBundleVersionData({
  baseDir,
  bundle,
  version,
  data,
}: WriteBundleVersionDataParams): Promise<void> {
  await lock.acquire();
  try {
    const filePath = getBundleVersionDataFilePath(baseDir, bundle, version);
    await fs.mkdir(path.dirname(filePath), { recursive: true });
    await writeFileAtomic(filePath, JSON.stringify(data, null, 2));
  } finally {
    lock.release();
  }
}

function getBundleFilePath(baseDir: string, bundle: string, version: string): string {
  const bundleName = normalizeBundleName(bundle);
  return path.join(baseDir, 'bundles', bundleName, `${version}.wvb`);
}

function getBundleVersionDataFilePath(baseDir: string, bundle: string, version: string): string {
  const bundleName = normalizeBundleName(bundle);
  return path.join(baseDir, 'bundles', bundleName, `${version}_data.json`);
}
