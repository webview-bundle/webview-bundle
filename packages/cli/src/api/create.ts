import type { HeadersConfig, IgnoreConfig } from '@wvb/config';
import { type Bundle, BundleBuilder, writeBundle } from '@wvb/node';
import { filterAsync, isNotNil } from 'es-toolkit';
import { isRegExp } from 'es-toolkit/predicate';
import fs from 'node:fs/promises';
import path from 'node:path';
import pm from 'picomatch';
import { glob } from 'tinyglobby';
import { c } from '../console.js';
import { formatByteLength } from '../format.js';
import { pathExists, toAbsolutePath, withWVBExtension } from '../fs.js';
import { isLogLevelAtLeast, type Logger, type LogLevel } from '../log.js';
import { ApiError } from './error.js';

export interface CreateParams {
  dir: string;
  outDir?: string;
  outFile: string;
  ignores?: IgnoreConfig[];
  headers?: HeadersConfig[];
  write?: boolean;
  overwrite?: boolean;
  cwd?: string;
  logLevel?: LogLevel;
  logger?: Logger;
}

/**
 * Create Webview Bundle archive.
 */
export async function create(params: CreateParams): Promise<Bundle> {
  const {
    dir: dirInput,
    outFile: outFileInput,
    outDir = '.wvb',
    ignores,
    headers,
    write = true,
    overwrite = true,
    cwd,
    logLevel = 'info',
    logger,
  } = params;
  const dir = toAbsolutePath(dirInput, cwd);
  const allFiles = await glob('**/*', {
    cwd: dir,
    onlyFiles: true,
    dot: true,
    debug: isLogLevelAtLeast(logLevel, 'debug'),
  });
  const ignoreInputs = ignores?.filter(isNotNil);
  const files = await filterAsync(allFiles, async file => {
    if (ignoreInputs == null || ignoreInputs.length === 0) {
      return true;
    }
    const ignored = await isFileIgnored(file, ignoreInputs);
    if (ignored) {
      logger?.debug(`File ignored: ${file}`);
    }
    return !ignored;
  });
  if (files.length === 0) {
    const message = 'No files to create bundle';
    logger?.error(message);
    throw new ApiError(message);
  }
  logger?.info(`Target ${files.length} files:`);
  const builder = new BundleBuilder();
  const headersInput = headers?.filter(isNotNil);
  for (const file of files) {
    const filepath = path.join(dir, file);
    const data = await fs.readFile(filepath);
    logger?.info(`- ${c.info(file)} ${c.bytes(formatByteLength(data.byteLength))}`);

    const headers = headersInput != null ? await getHeaders(file, headersInput) : undefined;
    builder.insertEntry(withSlash(file), data, undefined, headers);
  }

  const outFile = path.join(outDir, withWVBExtension(outFileInput));
  const outFilepath = toAbsolutePath(outFile, cwd);

  const bundle = builder.build();
  if (!write) {
    return bundle;
  }
  if (!overwrite) {
    if (await pathExists(outFilepath)) {
      const message = `Outfile already exists: ${c.bold(c.error(outFile))}`;
      logger?.error(message);
      throw new ApiError(message);
    }
  }
  await fs.mkdir(path.dirname(outFilepath), { recursive: true });
  const size = await writeBundle(bundle, outFilepath);
  logger?.info(`Output: ${c.bold(c.success(outFile))} ${c.bytes(formatByteLength(Number(size)))}`);
  return bundle;
}

async function isFileIgnored(file: string, ignoreInputs: IgnoreConfig[]): Promise<boolean> {
  if (ignoreInputs.length === 0) {
    return false;
  }
  for (const ignoreInput of ignoreInputs) {
    if (typeof ignoreInput === 'function') {
      return await ignoreInput(file);
    }
    if (Array.isArray(ignoreInput)) {
      for (const ignore of ignoreInput) {
        if (typeof ignore === 'string') {
          if (pm.isMatch(file, ignore)) {
            return true;
          }
        } else if (isRegExp(ignore)) {
          if (ignore.test(file)) {
            return true;
          }
        }
      }
    }
  }
  return false;
}

async function getHeaders(
  file: string,
  headerInputs: HeadersConfig[]
): Promise<Record<string, string> | undefined> {
  if (headerInputs.length === 0) {
    return undefined;
  }
  let headers = new Headers();
  for (const headerInput of headerInputs) {
    if (typeof headerInput === 'function') {
      const init = await headerInput(file);
      if (init != null) {
        headers = new Headers(init);
      }
    }
    const normalizedInput = Array.isArray(headerInput)
      ? headerInput
      : typeof headerInput === 'object' && headerInput != null
        ? Object.entries(headerInput)
        : [];
    for (const [pattern, init] of normalizedInput) {
      if (pm.isMatch(file, pattern)) {
        const appendHeaders = new Headers(init);
        for (const [key, value] of appendHeaders.entries()) {
          headers.set(key, value);
        }
      }
    }
  }
  const entries = [...headers.entries()];
  if (entries.length === 0) {
    return undefined;
  }
  return Object.fromEntries(entries);
}

function withSlash(file: string): string {
  return file.startsWith('/') ? file : `/${file}`;
}
