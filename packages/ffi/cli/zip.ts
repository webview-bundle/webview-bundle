import { createWriteStream } from 'node:fs';
import path from 'node:path';
import { glob } from 'tinyglobby';
import yazl from 'yazl';

interface Options {
  ignore?: string[];
}

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

  return new Promise<void>((resolve, reject) => {
    const output = createWriteStream(outputFile);
    const zipFile = new yazl.ZipFile();

    const stream = zipFile.outputStream.pipe(output);

    stream.on('close', () => resolve());
    stream.on('error', e => reject(e));

    for (const file of files) {
      const absolutePath = path.join(rootDir, file);
      zipFile.addFile(absolutePath, file);
    }
    zipFile.end();
  });
}
