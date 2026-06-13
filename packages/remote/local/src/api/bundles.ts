import type { Buffer } from 'node:buffer';
import { createReadStream, type ReadStream } from 'node:fs';
import fs from 'node:fs/promises';
import path from 'node:path';
import { z } from 'zod';
import { normalizeBundleName } from '../utils.js';

interface ReadBundleStreamParams {
  baseDir: string;
  bundle: string;
  version: string;
}

export function readBundleStream({ baseDir, bundle, version }: ReadBundleStreamParams): ReadStream {
  const filePath = getBundleFilePath(baseDir, bundle, version);
  return createReadStream(filePath);
}

export async function getBundleFileSize({
  baseDir,
  bundle,
  version,
}: ReadBundleStreamParams): Promise<number> {
  const filePath = getBundleFilePath(baseDir, bundle, version);
  const stats = await fs.stat(filePath);
  return stats.size;
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
  const filePath = getBundleFilePath(baseDir, bundle, version);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, data);
}

export const BundleMetadataFileSchema = z.object({
  integrity: z.string().optional(),
  signature: z.string().optional(),
});
export type BundleMetadataFile = z.infer<typeof BundleMetadataFileSchema>;

interface ReadBundleMetadataParams {
  baseDir: string;
  bundle: string;
  version: string;
}

export async function readBundleMetadata({
  baseDir,
  bundle,
  version,
}: ReadBundleMetadataParams): Promise<BundleMetadataFile | null> {
  const filePath = getBundleMetadataFilePath(baseDir, bundle, version);
  try {
    const raw = await fs.readFile(filePath, 'utf8');
    const parsed = BundleMetadataFileSchema.parse(JSON.parse(raw));
    return parsed;
  } catch {
    return null;
  }
}

interface WriteBundleMetadataParams {
  baseDir: string;
  bundle: string;
  version: string;
  metadata: BundleMetadataFile;
}

export async function writeBundleMetadata({
  baseDir,
  bundle,
  version,
  metadata,
}: WriteBundleMetadataParams): Promise<void> {
  const filePath = getBundleMetadataFilePath(baseDir, bundle, version);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, JSON.stringify(metadata, null, 2), 'utf8');
}

function getBundleFilePath(baseDir: string, bundle: string, version: string): string {
  const bundleName = normalizeBundleName(bundle);
  return path.join(baseDir, 'bundles', bundleName, `${bundleName}_${version}.wvb`);
}

function getBundleMetadataFilePath(baseDir: string, bundle: string, version: string): string {
  const bundleName = normalizeBundleName(bundle);
  return path.join(baseDir, 'bundles', bundleName, `${bundleName}_${version}.json`);
}
