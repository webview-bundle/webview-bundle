import fs from 'node:fs/promises';
import path from 'node:path';
import type { DeployBundleData, ResolvedSignatureConfig } from '@wvb/config/remote';
import {
  type BundleUpdate,
  type Current,
  CurrentSchema,
  generateUpdateDataETag,
  generateUpdateDataSignatures,
  type Update,
  UpdateSchema,
} from '@wvb/remote-base';
import { v7 as uuidv7 } from 'uuid';
import writeFileAtomic from 'write-file-atomic';
import { type BundleVersionData, getBundleFileSize, readBundleVersionData } from './bundles.js';
import { lock } from './lock.js';

interface ReadCurrentUpdateParams {
  baseDir: string;
  runtimeVersion: number;
  channel?: string;
}

export async function readCurrentUpdate({
  baseDir,
  runtimeVersion,
  channel,
}: ReadCurrentUpdateParams): Promise<Update | null> {
  try {
    const filePath = getCurrentFilePath(baseDir, runtimeVersion, channel);
    const raw = await fs.readFile(filePath, 'utf8');
    const current = CurrentSchema.parse(JSON.parse(raw));

    return await readUpdate({ baseDir, runtimeVersion, updateId: current.updateId, channel });
  } catch (e) {
    if (isFileNotFoundError(e)) {
      return null;
    }
    throw e;
  }
}

interface ReadUpdateParams {
  baseDir: string;
  updateId: string;
  runtimeVersion: number;
  channel?: string;
}

export async function readUpdate({
  baseDir,
  updateId,
  runtimeVersion,
  channel,
}: ReadUpdateParams): Promise<Update | null> {
  try {
    const filePath = getUpdateFilePath(baseDir, runtimeVersion, updateId, channel);
    const raw = await fs.readFile(filePath, 'utf8');

    return UpdateSchema.parse(JSON.parse(raw));
  } catch (e) {
    if (isFileNotFoundError(e)) {
      return null;
    }
    throw e;
  }
}

interface WriteUpdateParams {
  baseDir: string;
  bundles: DeployBundleData[];
  runtimeVersion?: number;
  channel?: string;
  signatures?: ResolvedSignatureConfig[];
  metadata?: Record<string, string>;
}

export async function writeUpdate({
  baseDir,
  bundles,
  channel,
  runtimeVersion = 1,
  signatures = [],
  metadata,
}: WriteUpdateParams): Promise<Update> {
  await lock.acquire();

  try {
    const update: Update = (await readCurrentUpdate({ baseDir, runtimeVersion, channel })) ?? {
      data: {
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
      const idx = update.data.bundles.findIndex(x => x.name === bundle.name);
      if (idx > -1) {
        update.data.bundles[idx] = bundle;
      } else {
        update.data.bundles.push(bundle);
      }
    }

    update.data.bundles.sort((a, b) => a.name.localeCompare(b.name));
    update.data.createdAt = new Date().toISOString();
    if (metadata != null) {
      update.data.metadata = metadata;
    }
    update.data.id = uuidv7();

    update.etag = generateUpdateDataETag(update.data);
    update.signatures = await generateUpdateDataSignatures(update.data, signatures);

    const filePath = getUpdateFilePath(baseDir, runtimeVersion, update.data.id, channel);
    await fs.mkdir(path.dirname(filePath), { recursive: true });
    await writeFileAtomic(filePath, JSON.stringify(update, null, 2));

    const currentFilePath = getCurrentFilePath(baseDir, runtimeVersion, channel);
    const currentFile: Current = {
      updateId: update.data.id,
      currentAt: new Date().toISOString(),
    };
    await fs.mkdir(path.dirname(currentFilePath), { recursive: true });
    await writeFileAtomic(currentFilePath, JSON.stringify(currentFile, null, 2));

    return update;
  } finally {
    lock.release();
  }
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

function isFileNotFoundError(e: unknown): e is NodeJS.ErrnoException {
  return e instanceof Error && 'code' in e && e.code === 'ENOENT';
}

function getCurrentFilePath(baseDir: string, runtimeVersion: number, channel?: string): string {
  if (channel != null) {
    return path.join(
      baseDir,
      'channels',
      channel,
      'updates',
      String(runtimeVersion),
      'current.json'
    );
  }
  return path.join(baseDir, 'updates', String(runtimeVersion), 'current.json');
}

function getUpdateFilePath(
  baseDir: string,
  runtimeVersion: number,
  updateId: string,
  channel?: string
): string {
  if (channel != null) {
    return path.join(
      baseDir,
      'channels',
      channel,
      'updates',
      String(runtimeVersion),
      `${updateId}.json`
    );
  }
  return path.join(baseDir, 'updates', String(runtimeVersion), `${updateId}.json`);
}
