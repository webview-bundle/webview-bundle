import { type Bundle, type IndexEntry, readBundle } from '@wvb/node';
import fs from 'node:fs/promises';
import path from 'node:path';
import type { Logger } from '../log.js';
import { c } from '../console.js';
import { formatByteLength } from '../format.js';
import { pathExists, toAbsolutePath, withWVBExtension } from '../fs.js';
import { ApiError } from './error.js';

export interface ExtractParams {
  file: string;
  outDir?: string;
  cwd?: string;
  write?: boolean;
  clean?: boolean;
  logger?: Logger;
}

/**
 * Extract Webview Bundle files.
 */
export async function extract(params: ExtractParams): Promise<Bundle> {
  const { file, outDir: outDirInput, cwd, write = true, clean = false, logger } = params;

  const filepath = toAbsolutePath(withWVBExtension(file), cwd);
  if (!(await pathExists(filepath))) {
    const message = `File does not exist: ${filepath}`;
    logger?.error(message);
    throw new ApiError(message);
  }
  const bundle = await readBundle(filepath);
  logger?.info(`Webview Bundle info for ${c.info(filepath)}`);
  logger?.info(`Version: ${c.bold(c.info(bundle.descriptor().header().version()))}`);
  logger?.info(`Entries:`);
  const entries = Object.entries(bundle.descriptor().index().entries());
  sortEntries(entries);
  for (const [p, entry] of entries) {
    const data = bundle.getData(p)!;
    const bytes = formatByteLength(data.byteLength);
    logger?.info(`${c.info(p)} ${c.bytes(bytes)}`);
    logger?.info(`  ${c.header(['content-type', entry.contentType])}`);
    logger?.info(`  ${c.header(['content-length', String(entry.contentLength)])}`);
    for (const h of Object.entries(entry.headers)) {
      logger?.info(`  ${c.header(h)}`);
    }
  }
  if (!write) {
    return bundle;
  }
  const outDirPath = toAbsolutePath(outDirInput ?? path.basename(filepath, '.wvb'), cwd);
  if (await pathExists(outDirPath)) {
    if (!clean) {
      const message = `Output directory already exists: ${outDirPath}`;
      logger?.warn(message);
      throw new ApiError(message);
    }
    await fs.rm(outDirPath, { recursive: true });
  }
  const entryPaths = Object.keys(bundle.descriptor().index().entries());
  for (const p of entryPaths) {
    const filepath = path.join(outDirPath, p);
    await fs.mkdir(path.dirname(filepath), { recursive: true });
    await fs.writeFile(filepath, bundle.getData(p)!);
  }
  logger?.info(`Extract completed: ${c.bold(c.success(outDirPath))}`);
  return bundle;
}

function sortEntries(entries: Array<[string, IndexEntry]>): void {
  entries.sort((a, b) => {
    if (a[0] < b[0]) {
      return -1;
    } else if (a[0] > b[0]) {
      return 1;
    }
    return 0;
  });
}
