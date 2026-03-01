import { type BundleManifestData, type ListRemoteBundleInfo, Remote } from '@wvb/node';
import { MultiBar, Presets, type SingleBar } from 'cli-progress';
import { filterAsync } from 'es-toolkit';
import { limitAsync } from 'es-toolkit/array';
import { isRegExp } from 'es-toolkit/predicate';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import pm from 'picomatch';
import type { Logger } from '../log.js';
import { c } from '../console.js';
import { pathExists, toAbsolutePath } from '../fs.js';
import { coerceArray } from '../utils/coerce.js';
import { ApiError } from './error.js';

type RemoteBundleMatches =
  | string
  | RegExp
  | Array<string | RegExp>
  | ((info: ListRemoteBundleInfo) => boolean | Promise<boolean>);

export interface BuiltinParams {
  remoteEndpoint: string;
  dir?: string;
  include?: RemoteBundleMatches[];
  exclude?: RemoteBundleMatches[];
  channel?: string;
  clean?: boolean;
  cwd?: string;
  write?: boolean;
  logger?: Logger;
  concurrency?: number;
  progress?: boolean;
}

/**
 * Install builtin Webview Bundles from remote.
 */
export async function builtin(params: BuiltinParams): Promise<BundleManifestData> {
  const {
    remoteEndpoint,
    dir: dirInput = path.join('.wvb', 'builtin'),
    include,
    exclude,
    channel,
    clean = true,
    write = true,
    cwd,
    logger,
    concurrency = defaultConcurrency(),
    progress: showProgress = false,
  } = params;
  const dir = toAbsolutePath(dirInput, cwd);
  if (clean && write && (await pathExists(dir))) {
    await fs.rm(dir, { recursive: true });
  }
  let remote = new Remote(remoteEndpoint);
  const remoteBundles = await remote.listBundles(channel);
  if (remoteBundles.length > 0) {
    logger?.info(channel != null ? `Remote bundles (${channel}):` : 'Remote bundles:');
    for (const remoteBundle of remoteBundles) {
      logger?.info(`  ${c.info(remoteBundle.name)}: ${c.bold(c.info(remoteBundle.version))}`);
    }
  }

  const progress = showProgress
    ? new MultiBar(
        {
          format: `{bundleName} ${c.progress('{bar}')} {percentage}% ({value}/{total})`,
          clearOnComplete: false,
          // https://github.com/npkgz/cli-progress/issues/126
          gracefulExit: false,
        },
        Presets.shades_grey
      )
    : null;
  const progressBars = new Map<string, SingleBar>();
  remote = new Remote(remoteEndpoint, {
    onDownload: ({ downloadedBytes, totalBytes, endpoint }) => {
      if (progress == null) {
        return;
      }
      const bundleName = findBundleNameFromEndpoint(endpoint);
      if (bundleName == null) {
        return;
      }
      const bar =
        progressBars.get(bundleName) ??
        progress.create(totalBytes, downloadedBytes, { bundleName });
      if (bar.isActive) {
        bar.update(downloadedBytes);
      }
      progressBars.set(bundleName, bar);
    },
  });

  const manifest: BundleManifestData = {
    manifestVersion: 1,
    entries: {},
  };

  const install = limitAsync(async (remoteBundle: ListRemoteBundleInfo) => {
    const bundleName = remoteBundle.name;
    try {
      const [{ version, etag, integrity, signature, lastModified }, , buffer] =
        await remote.download(remoteBundle.name, channel);
      manifest.entries[bundleName] = {
        versions: {
          [version]: {
            etag,
            integrity,
            signature,
            lastModified,
          },
        },
        currentVersion: version,
      };

      if (write) {
        const filename = `${bundleName}_${version}.wvb`;
        const filepath = path.join(dir, bundleName, filename);
        await fs.mkdir(path.dirname(filepath), { recursive: true });
        await fs.writeFile(filepath, buffer);
      }

      return { success: true, bundleName } as const;
    } catch (error) {
      return { success: false, bundleName, error } as const;
    } finally {
      progressBars.get(bundleName)?.stop();
    }
  }, concurrency);

  const remoteBundlesToDownload = await filterAsync(remoteBundles, async remoteBundle => {
    const shouldInclude = include != null ? await isInMatches(remoteBundle, include, true) : true;
    if (!shouldInclude) {
      logger?.debug(`Remote bundle not included: ${remoteBundle.name}`);
      return false;
    }
    const shouldExclude = exclude != null ? await isInMatches(remoteBundle, exclude, false) : false;
    if (shouldExclude) {
      logger?.debug(`Remote bundle excluded: ${remoteBundle.name}`);
      return false;
    }
    return true;
  });

  if (remoteBundlesToDownload.length === 0) {
    const message = 'No remote bundles to install.';
    logger?.error(message);
    throw new ApiError(message);
  }

  const result = await Promise.all(remoteBundlesToDownload.map(install));
  progress?.stop();

  const failures = result.filter(x => !x.success);
  if (failures.length > 0) {
    for (const failure of failures) {
      logger?.error(`"${c.bold(failure.bundleName)}" install failed: {error}`, {
        error: failure.error,
      });
    }
    throw new ApiError(
      `Install failed: ${failures.map(x => x.bundleName).join(', ')}`,
      failures.map(x => x.error)
    );
  }

  const manifestFilepath = path.join(dir, 'manifest.json');
  if (write) {
    await fs.writeFile(manifestFilepath, JSON.stringify(manifest, null, 2));
    logger?.info(`Manifest saved: ${c.bold(c.success(manifestFilepath))}`);
    logger?.info(`Builtin bundles installed: ${c.bold(c.success(dir))}`);
  }

  return manifest;
}

function defaultConcurrency() {
  const cpus = os.availableParallelism?.() ?? os.cpus().length - 1;
  return Math.max(1, Math.min(cpus, 8));
}

function findBundleNameFromEndpoint(endpoint: string): string | undefined {
  try {
    const url = new URL(endpoint);
    const segments = url.pathname.slice(1).split('/');
    const bundlesIndex = segments.findIndex(x => x === 'bundles');
    return bundlesIndex > -1 ? segments[bundlesIndex + 1] : undefined;
  } catch {
    return undefined;
  }
}

async function isInMatches(
  info: ListRemoteBundleInfo,
  matches: RemoteBundleMatches[],
  onEmpty: boolean
): Promise<boolean> {
  const filteredMatches = matches.filter(x => (Array.isArray(x) ? x.length > 0 : true));
  if (filteredMatches.length === 0) {
    return onEmpty;
  }
  for (const match of filteredMatches) {
    if (typeof match === 'function') {
      if (await match(info)) {
        return true;
      }
    }
    const predicates = coerceArray(match);
    for (const predicate of predicates) {
      if (typeof predicate === 'string') {
        if (pm.isMatch(info.name, predicate)) {
          return true;
        }
      }
      if (isRegExp(predicate)) {
        if (predicate.test(info.name)) {
          return true;
        }
      }
    }
  }
  return false;
}
