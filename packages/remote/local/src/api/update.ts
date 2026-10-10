import { Buffer } from 'node:buffer';
import fs from 'node:fs/promises';
import path from 'node:path';
import {
  type DeployBundleData,
  getSignatureValue,
  type ResolvedSignatureConfig,
} from '@wvb/config/remote';
import { v7 as uuidv7 } from 'uuid';
import writeFileAtomic from 'write-file-atomic';
import { z } from 'zod';
import { type BundleVersionData, getBundleFileSize, readBundleVersionData } from './bundles.js';
import { lock } from './lock.js';

export const BundleUpdateSchema = z.object({
  name: z.string(),
  version: z.string(),
  integrity: z.string().optional(),
  metadata: z.record(z.string(), z.string()).optional(),
});
export type BundleUpdate = z.infer<typeof BundleUpdateSchema>;

export const UpdateSchema = z.object({
  id: z.uuidv7(),
  createdAt: z.iso.datetime(),
  runtimeVersion: z.int(),
  bundles: BundleUpdateSchema.array(),
  metadata: z.record(z.string(), z.string()).optional(),
});
export type Update = z.infer<typeof UpdateSchema>;

export const UpdateSignatureSchema = z.object({
  id: z.string(),
  alg: z.string(),
  sig: z.string(),
});
export type UpdateSignature = z.infer<typeof UpdateSignatureSchema>;

export const UpdateFileSchema = z.object({
  update: UpdateSchema,
  signatures: UpdateSignatureSchema.array(),
});
export type UpdateFile = z.infer<typeof UpdateFileSchema>;

interface ReadUpdateFileParams {
  baseDir: string;
  channel?: string;
}

export async function readUpdateFile({
  baseDir,
  channel,
}: ReadUpdateFileParams): Promise<UpdateFile | null> {
  try {
    const filePath = getUpdateFilePath(baseDir, channel);
    const raw = await fs.readFile(filePath, 'utf8');

    return UpdateFileSchema.parse(JSON.parse(raw));
  } catch (e) {
    if (isFileNotFoundError(e)) {
      return null;
    }
    throw e;
  }
}

interface WriteUpdateFileParams {
  baseDir: string;
  bundles: DeployBundleData[];
  runtimeVersion?: number;
  channel?: string;
  signatures?: ResolvedSignatureConfig[];
  metadata?: Record<string, string>;
}

export async function writeUpdateFile({
  baseDir,
  bundles,
  channel,
  runtimeVersion = 1,
  signatures = [],
  metadata,
}: WriteUpdateFileParams): Promise<UpdateFile> {
  await lock.acquire();

  try {
    const { update }: UpdateFile = (await readUpdateFile({ baseDir, channel })) ?? {
      update: {
        id: '',
        createdAt: new Date().toISOString(),
        runtimeVersion,
        bundles: [],
      },
      signatures: [],
    };

    const updateBundles = await Promise.all(
      bundles.map(async bundle => {
        const bundleSize = await getBundleFileSize({
          baseDir,
          bundle: bundle.name,
          version: bundle.version,
        });
        if (bundleSize == null) {
          throw new Error(`Bundle not exists: name=${bundle.name}, version=${bundle.version}`);
        }

        const versionData = await readBundleVersionData({
          baseDir,
          bundle: bundle.name,
          version: bundle.version,
        });
        return bundleUpdateFromVersionData(bundle, versionData);
      })
    );

    for (const bundle of updateBundles) {
      const idx = update.bundles.findIndex(x => x.name === bundle.name);
      if (idx > -1) {
        update.bundles[idx] = bundle;
      } else {
        update.bundles.push(bundle);
      }
    }

    update.bundles.sort((a, b) => a.name.localeCompare(b.name));
    update.createdAt = new Date().toISOString();
    if (metadata != null) {
      update.metadata = metadata;
    }
    update.id = uuidv7();

    const updateSignatures = await signUpdate(update, signatures);

    const updateFile: UpdateFile = {
      update,
      signatures: updateSignatures,
    };

    const filePath = getUpdateFilePath(baseDir, channel);
    await fs.mkdir(path.dirname(filePath), { recursive: true });
    await writeFileAtomic(filePath, JSON.stringify(updateFile, null, 2));

    return updateFile;
  } finally {
    lock.release();
  }
}

async function signUpdate(
  update: Update,
  signatures: ResolvedSignatureConfig[]
): Promise<UpdateSignature[]> {
  const message = Buffer.from(stringifyUpdate(update), 'utf8');
  return await Promise.all(signatures.map(signature => getSignatureValue(signature, message)));
}

function bundleUpdateFromVersionData(
  bundle: DeployBundleData,
  versionData: BundleVersionData | null
): BundleUpdate {
  return {
    name: bundle.name,
    version: bundle.version,
    ...(versionData?.integrity !== undefined ? { integrity: versionData.integrity } : {}),
    ...(versionData?.metadata !== undefined ? { metadata: versionData.metadata } : {}),
  };
}

/** Serializes an update deterministically for signing and serving. */
export function stringifyUpdate(update: Update): string {
  return JSON.stringify(update, (_key, value: unknown) => {
    if (value == null || typeof value !== 'object' || Array.isArray(value)) {
      return value;
    }

    const record = value as Record<string, unknown>;
    return Object.fromEntries(
      Object.keys(record)
        .sort()
        .map(key => [key, record[key]])
    );
  });
}

function isFileNotFoundError(e: unknown): e is NodeJS.ErrnoException {
  return e instanceof Error && 'code' in e && e.code === 'ENOENT';
}

function getUpdateFilePath(baseDir: string, channel?: string): string {
  if (channel != null) {
    return path.join(baseDir, 'updates', 'channels', channel, 'update.json');
  }
  return path.join(baseDir, 'updates', 'update.json');
}
