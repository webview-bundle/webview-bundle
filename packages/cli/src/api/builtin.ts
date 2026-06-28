import { Buffer } from 'node:buffer';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import type {
  BuiltinBundleMatches,
  BuiltinLocalTargetConfig,
  BuiltinRemoteTargetConfig,
  BuiltinTarget,
} from '@wvb/config';
import { makeIntegrity, signSignature } from '@wvb/config/remote';
import {
  type Bundle,
  type BundleManifestData,
  Remote,
  type RemoteOptions,
  readBundle,
  writeBundleIntoBuffer,
} from '@wvb/node';
import { MultiBar, Presets, type SingleBar } from 'cli-progress';
import { chunk, filterAsync } from 'es-toolkit';
import { isRegExp } from 'es-toolkit/predicate';
import pm from 'picomatch';
import { glob } from 'tinyglobby';
import {
  type ResolvedConfig,
  resolveBundleName,
  resolveConfig,
  resolveOutFile,
  resolveVersion,
} from '../config.js';
import { c } from '../console.js';
import { pathExists, toAbsolutePath, withWvbExtension } from '../fs.js';
import { isLogLevelAtLeast, type Logger, type LogLevel } from '../log.js';
import {
  type AndroidNoCompressStatus,
  addIosFolderReference,
  checkAndroidNoCompress,
  type IosAddFolderReferenceStatus,
} from '../mobile.js';
import { coerceArray } from '../utils/coerce.js';
import { ApiError } from './error.js';
import { pack } from './pack.js';

export interface BuiltinParams {
  target: BuiltinTarget;
  dir?: string;
  include?: BuiltinBundleMatches[];
  exclude?: BuiltinBundleMatches[];
  channel?: string;
  clean?: boolean;
  cwd?: string;
  write?: boolean;
  logLevel?: LogLevel;
  logger?: Logger;
  progress?: boolean;
  android?: {
    dir: string;
    checkNoCompress?: boolean;
  };
  ios?: {
    dir: string;
    /** This only works if using a Tuist project. */
    addProjectFolderReference?: boolean;
  };
}

export interface BuiltinResult {
  manifest: BundleManifestData;
  android?: {
    noCompressStatus?: AndroidNoCompressStatus;
  };
  ios?: {
    addFolderReferenceStatus?: IosAddFolderReferenceStatus;
  };
}

/**
 * Install builtin Webview Bundles from remote and/or local files.
 */
export async function builtin(params: BuiltinParams): Promise<BuiltinResult> {
  const {
    target,
    dir: dirInput = path.join('.wvb', 'builtin', 'bundles'),
    include,
    exclude,
    channel,
    clean = true,
    write = true,
    cwd = process.cwd(),
    logLevel,
    logger,
    progress: showProgress = false,
    android,
    ios,
  } = params;

  const dir = toAbsolutePath(dirInput, cwd);

  if (clean && write && (await pathExists(dir))) {
    await fs.rm(dir, { recursive: true });
  }

  const manifest: BundleManifestData = {
    manifestVersion: 1,
    entries: {},
  };
  const progress = showProgress && target.type === 'remote' ? buildProgress() : null;

  const getLoadedBundles = (): AsyncGenerator<LoadedBundle[]> => {
    switch (target.type) {
      case 'remote':
        return loadRemoteBundles(target, {
          channel,
          onDownload: progress?.onDownload,
          include,
          exclude,
          logLevel,
          logger,
        });
      case 'local':
        return loadLocalBundles(cwd, target, {
          include,
          exclude,
          write,
          logLevel,
          logger,
        });
    }
  };

  type InstallResult =
    | { success: true; name: string; version: string }
    | { success: false; name: string; version: string; error: Error };

  const install = async (bundle: LoadedBundle): Promise<InstallResult> => {
    try {
      manifest.entries[bundle.name] = {
        versions: {
          [bundle.version]: {
            etag: bundle.etag,
            integrity: bundle.integrity,
            signature: bundle.signature,
            lastModified: bundle.lastModified,
          },
        },
        currentVersion: bundle.version,
      };

      if (write) {
        const filename = `${bundle.name}_${bundle.version}.wvb`;
        const filepath = path.join(dir, bundle.name, filename);
        await fs.mkdir(path.dirname(filepath), { recursive: true });
        await fs.writeFile(filepath, bundle.data);
      }

      return { success: true, name: bundle.name, version: bundle.version };
    } catch (e: unknown) {
      delete manifest.entries[bundle.name];
      return { success: false, name: bundle.name, version: bundle.version, error: e as Error };
    }
  };

  const installResults: InstallResult[] = [];

  for await (const loadedBundles of getLoadedBundles()) {
    const results = await Promise.all(loadedBundles.map(install));
    installResults.push(...results);
  }
  progress?.progress.stop();

  const failures = installResults.filter(x => !x.success);
  if (failures.length > 0) {
    for (const failure of failures) {
      logger?.error(`"${c.bold(failure.name)}" install failed: {error}`, {
        error: failure.error,
      });
    }
    throw new ApiError(
      `Install failed: ${failures.map(x => `${x.name}@${x.version}`).join(', ')}`,
      failures.map(x => x.error)
    );
  }

  const manifestFilepath = path.join(dir, 'manifest.json');
  if (write) {
    // `dir` is created lazily per-bundle by install(); when nothing was installed (empty workspace
    // glob, or every bundle filtered out) it won't exist yet — and `clean` may have removed it — so
    // ensure it exists before writing the manifest to avoid an ENOENT.
    await fs.mkdir(dir, { recursive: true });
    await fs.writeFile(manifestFilepath, JSON.stringify(manifest, null, 2));
    logger?.info(`Manifest saved: ${c.bold(c.success(manifestFilepath))}`);
    logger?.info(`Builtin bundles installed: ${c.bold(c.success(dir))}`);
  }

  let androidNoCompressStatus: AndroidNoCompressStatus | undefined;
  if (android?.checkNoCompress === true) {
    androidNoCompressStatus = await checkAndroidNoCompress(android.dir);
    if (androidNoCompressStatus === 'missing') {
      logger?.warn(
        "'.wvb' assets may be re-compressed in the APK. Add to your module's build.gradle(.kts):\n" +
          '  android { androidResources { noCompress += "wvb" } }\n' +
          'And extract the assets to a filesystem dir at runtime.'
      );
    }
  }

  let iosAddFolderReferenceStatus: IosAddFolderReferenceStatus | undefined;
  if (ios?.addProjectFolderReference === true) {
    const projectSwift = path.join(ios.dir, 'Project.swift');
    const bundlesDir = path.relative(ios.dir, dir);
    iosAddFolderReferenceStatus = await addIosFolderReference(ios.dir, bundlesDir);

    switch (iosAddFolderReferenceStatus) {
      case 'added':
        logger?.info(
          `Added \`folderReference(path: "./${bundlesDir}")\` to ${projectSwift}. ` +
            'Run `tuist generate` to regenerate the Xcode project.'
        );
        break;
      case 'already':
        logger?.info(`${projectSwift} already references "${bundlesDir}".`);
        break;
      case 'no-resources':
        break;
      case 'not-found':
        logger?.warn(
          `Project.swift not found in ${ios.dir}. Add "${dir}" to your iOS target as a ` +
            '"FOLDER REFERENCE" so the per-bundle subdirectories are preserved.'
        );
        break;
    }
  }

  return {
    manifest,
    android:
      android != null
        ? {
            noCompressStatus: androidNoCompressStatus,
          }
        : undefined,
    ios:
      ios != null
        ? {
            addFolderReferenceStatus: iosAddFolderReferenceStatus,
          }
        : undefined,
  };
}

function buildProgress() {
  const progress = new MultiBar(
    {
      format: `{bundleName} ${c.progress('{bar}')} {percentage}% ({value}/{total})`,
      clearOnComplete: false,
      // https://github.com/npkgz/cli-progress/issues/126
      gracefulExit: false,
    },
    Presets.shades_grey
  );
  const progressBars = new Map<string, SingleBar>();

  const onDownload: NonNullable<RemoteOptions['onDownload']> = ({
    downloadedBytes,
    totalBytes,
    endpoint,
  }) => {
    if (progress == null || totalBytes == null) {
      return;
    }
    const bundleName = findBundleNameFromEndpoint(endpoint);
    if (bundleName == null) {
      return;
    }
    const bar =
      progressBars.get(bundleName) ?? progress.create(totalBytes, downloadedBytes, { bundleName });
    if (bar.isActive) {
      bar.update(downloadedBytes);
    }
    progressBars.set(bundleName, bar);
  };

  return { progress, progressBars, onDownload };
}

interface LoadedBundle {
  name: string;
  version: string;
  data: Buffer;
  etag?: string;
  signature?: string;
  integrity?: string;
  lastModified?: string;
}

async function* loadLocalBundles(
  cwd: string,
  target: BuiltinLocalTargetConfig,
  options: {
    include?: BuiltinBundleMatches[];
    exclude?: BuiltinBundleMatches[];
    write?: boolean;
    logLevel?: LogLevel;
    logger?: Logger;
  }
): AsyncGenerator<LoadedBundle[]> {
  const workspaces =
    typeof target.workspaces === 'function' ? await target.workspaces() : target.workspaces;
  const resolvedWorkspaces = await resolveLocalWorkspaces(
    cwd,
    workspaces,
    options.logLevel != null ? isLogLevelAtLeast(options.logLevel, 'debug') : false
  );

  options.logger?.info(`Found ${resolvedWorkspaces.length} local workspaces`);
  for (const resolvedWorkspace of resolvedWorkspaces) {
    options.logger?.info(`- ${resolvedWorkspace.dir}`);
  }

  if (resolvedWorkspaces.length === 0) {
    options.logger?.warn('No local workspaces to install.');
    return;
  }

  for (const w of resolvedWorkspaces) {
    let bundle: Bundle;

    const outFile = resolveOutFile(w.config);

    if (outFile == null) {
      const message = `Out file is not specified. Set "pack.outFile" in the config file. (from "${w.dir}" workspace)`;
      options.logger?.error(message);
      throw new ApiError(message);
    }

    const outFilePath = withWvbExtension(toAbsolutePath(outFile, w.config.root));

    const bundleName = await resolveBundleName(w.config, target.bundleName, {
      file: outFilePath,
    });
    if (bundleName == null) {
      const message = `Bundle name is required for this operation. (from "${w.dir}" workspace)`;
      options.logger?.error(message);
      throw new ApiError(message);
    }

    const version = await resolveVersion(w.config, target.version);
    if (version == null) {
      const message = `Version is required for this operation. (from "${w.dir}" workspace)`;
      options.logger?.error(message);
      throw new ApiError(message);
    }

    const shouldInclude =
      options.include != null
        ? await isInMatches(bundleName, version, options.include, true)
        : true;
    if (!shouldInclude) {
      options.logger?.debug(`Local bundle not included: ${bundleName}`);
      continue;
    }
    const shouldExclude =
      options.exclude != null
        ? await isInMatches(bundleName, version, options.exclude, false)
        : false;
    if (shouldExclude) {
      options.logger?.debug(`Local bundle excluded: ${bundleName}`);
      continue;
    }

    const shouldPack = target.packBeforeInstall ?? true;
    if (shouldPack) {
      const srcDir = w.config.pack?.srcDir ?? './dist';
      const overwrite = w.config.pack?.overwrite ?? true;

      const packResult = await pack({
        srcDir,
        outFile,
        overwrite,
        // Honor the caller's `write` flag so `--no-write` is a true simulation (pack still builds the
        // bundle in memory, it just doesn't touch disk).
        write: options.write ?? true,
        cwd: w.config.root,
        logLevel: options.logLevel,
        logger: options.logger,
      });
      bundle = packResult.bundle;
    } else {
      bundle = await readBundle(outFilePath);
    }

    let lastModified: string | undefined;
    try {
      const stat = await fs.stat(outFilePath);
      // HTTP `Last-Modified` header format (RFC 7231 IMF-fixdate), e.g. "Wed, 21 Oct 2015 07:28:00 GMT".
      lastModified = stat.mtime.toUTCString();
    } catch {
      // bundle was not written so leave `lastModified` undefined.
    }

    const loaded: LoadedBundle = {
      name: bundleName,
      version,
      data: writeBundleIntoBuffer(bundle),
      lastModified,
    };

    if (target.integrity !== false) {
      const opts =
        target.integrity == null || typeof target.integrity === 'boolean' ? {} : target.integrity;
      loaded.integrity = await makeIntegrity(opts, loaded.data);
    }

    if (target.signature != null) {
      if (loaded.integrity == null) {
        const message = `Cannot make signature without integrity. Make sure the integrity option is enabled.`;
        options.logger?.error(message);
        throw new ApiError(message);
      }
      loaded.signature = await signSignature(
        target.signature,
        Buffer.from(loaded.integrity, 'utf8')
      );
    }

    yield [loaded];
  }
}

interface ResolvedWorkspace {
  absoluteDir: string;
  dir: string;
  config: ResolvedConfig;
}

async function resolveLocalWorkspaces(
  cwd: string,
  workspaces: string[],
  debug?: boolean
): Promise<ResolvedWorkspace[]> {
  const patterns = workspaces.map(x => {
    if (x.endsWith('package.json')) {
      return x;
    }
    return x.endsWith('/') ? `${x}package.json` : `${x}/package.json`;
  });

  // We only want to resolve directories which includes "package.json".
  const packageFiles = await glob(patterns, {
    absolute: true,
    cwd,
    onlyFiles: true,
    debug,
  });
  const resolved: ResolvedWorkspace[] = await Promise.all(
    packageFiles.map(async packageFile => {
      const absoluteDir = path.dirname(packageFile);
      const config = await resolveConfig({
        root: absoluteDir,
      });
      return { absoluteDir, dir: path.relative(cwd, absoluteDir), config };
    })
  );

  return resolved;
}

async function* loadRemoteBundles(
  target: BuiltinRemoteTargetConfig,
  options: {
    onDownload?: RemoteOptions['onDownload'];
    channel?: string;
    include?: BuiltinBundleMatches[];
    exclude?: BuiltinBundleMatches[];
    logLevel?: LogLevel;
    logger?: Logger;
  }
): AsyncGenerator<LoadedBundle[]> {
  const remoteEndpoint = target.endpoint;
  if (remoteEndpoint == null) {
    const message = 'Remote endpoint is required.';
    options.logger?.error(message);
    throw new ApiError(message);
  }

  let remote = new Remote(remoteEndpoint, {
    http: target.download?.http,
  });
  const channel = options.channel;
  const allRemoteBundles = await remote.listBundles(channel);
  const remoteBundles = await filterAsync(allRemoteBundles, async remoteBundle => {
    const shouldInclude =
      options.include != null
        ? await isInMatches(remoteBundle.name, remoteBundle.version, options.include, true)
        : true;
    if (!shouldInclude) {
      options.logger?.debug(`Remote bundle not included: ${remoteBundle.name}`);
      return false;
    }

    const shouldExclude =
      options.exclude != null
        ? await isInMatches(remoteBundle.name, remoteBundle.version, options.exclude, false)
        : false;
    if (shouldExclude) {
      options.logger?.debug(`Remote bundle excluded: ${remoteBundle.name}`);
      return false;
    }
    return true;
  });

  if (remoteBundles.length === 0) {
    options.logger?.warn('No remote bundles to install.');
    return;
  }

  options.logger?.info(channel != null ? `Remote bundles (${channel}):` : 'Remote bundles:');
  for (const remoteBundle of remoteBundles) {
    options.logger?.info(`  ${c.info(remoteBundle.name)}: ${c.bold(c.info(remoteBundle.version))}`);
  }

  remote = new Remote(remoteEndpoint, {
    http: target.download?.http,
    onDownload: options.onDownload,
  });

  const concurrency = Math.max(target.download?.concurrency ?? defaultDownloadConcurrency(), 1);

  for (const chunks of chunk(remoteBundles, concurrency)) {
    const downloaded = await Promise.all(
      chunks.map(async remoteBundle => {
        const [info, _, data] = await remote.download(remoteBundle.name, channel);
        const loaded: LoadedBundle = {
          ...info,
          data,
        };
        return loaded;
      })
    );
    yield downloaded;
  }
}

function defaultDownloadConcurrency() {
  const cpus = os.availableParallelism?.() ?? os.cpus().length - 1;
  return Math.max(1, Math.min(cpus, 8));
}

function findBundleNameFromEndpoint(endpoint: string): string | undefined {
  try {
    const url = new URL(endpoint);
    const segments = url.pathname.slice(1).split('/');
    const bundlesIndex = segments.indexOf('bundles');
    return bundlesIndex > -1 ? segments[bundlesIndex + 1] : undefined;
  } catch {
    return undefined;
  }
}

async function isInMatches(
  bundleName: string,
  version: string,
  matches: BuiltinBundleMatches[],
  onEmpty: boolean
): Promise<boolean> {
  const filteredMatches = matches.filter(x => (Array.isArray(x) ? x.length > 0 : true));
  if (filteredMatches.length === 0) {
    return onEmpty;
  }
  for (const match of filteredMatches) {
    if (typeof match === 'function') {
      if (await match({ name: bundleName, version })) {
        return true;
      }
    }
    const predicates = coerceArray(match);
    for (const predicate of predicates) {
      if (typeof predicate === 'string') {
        // picomatch throws on an empty pattern; treat "" as a no-op predicate.
        if (predicate.length > 0 && pm.isMatch(bundleName, predicate)) {
          return true;
        }
      }
      if (isRegExp(predicate)) {
        if (predicate.test(bundleName)) {
          return true;
        }
      }
    }
  }
  return false;
}
