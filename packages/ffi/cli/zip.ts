import { createWriteStream } from 'node:fs';
import fs from 'node:fs/promises';
import path from 'node:path';
import { glob } from 'tinyglobby';
import yazl from 'yazl';

interface Options {
  ignore?: string[];
}

/**
 * Fixed timestamp so the archive is reproducible: identical file contents always produce identical
 * bytes (hence the same checksum), regardless of when the files were built. Must be >= 1980 (the
 * ZIP/DOS time floor). yazl converts this with local-time getters, so builds should run in a fixed
 * timezone (GitHub runners are UTC) for the bytes to match across machines.
 */
const FIXED_MTIME = new Date('2000-01-01T00:00:00.000Z');

export async function zip(
  outputFile: string,
  rootDir: string,
  patterns: string[],
  options?: Options
): Promise<void> {
  const files = await glob(patterns, {
    cwd: rootDir,
    onlyFiles: true,
    dot: true,
    ignore: options?.ignore,
  });
  // Stable entry order — yazl writes entries in the order they are added.
  files.sort();

  const entries = await Promise.all(
    files.map(async file => {
      const absolutePath = path.join(rootDir, file);
      const { mode } = await fs.stat(absolutePath);
      // Normalize permissions to two canonical modes, preserving only the "executable" bit, so the
      // stored attributes don't depend on the builder's umask.
      return { absolutePath, file, mode: (mode & 0o111) !== 0 ? 0o755 : 0o644 };
    })
  );

  return new Promise<void>((resolve, reject) => {
    const output = createWriteStream(outputFile);
    const zipFile = new yazl.ZipFile();

    const stream = zipFile.outputStream.pipe(output);

    stream.on('close', () => resolve());
    stream.on('error', e => reject(e));

    for (const { absolutePath, file, mode } of entries) {
      zipFile.addFile(absolutePath, file, { mtime: FIXED_MTIME, mode });
    }
    zipFile.end();
  });
}
