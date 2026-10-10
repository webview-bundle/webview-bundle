import fs from 'node:fs/promises';
import path from 'node:path';
import { generateCode } from './src/code.js';

const inputs = await fs.readdir(path.join(import.meta.dirname, 'lambda'));
for (const input of inputs) {
  const inputPath = path.join(import.meta.dirname, 'lambda', input);
  const codes = await generateCode(inputPath, {
    platform: 'node',
    target: 'node22',
    format: 'esm',
    external: [/^@aws-sdk\//],
    define: {
      __CONFIG__: JSON.stringify({}),
    },
    minify: false,
  });
  const outdir = path.join(
    import.meta.dirname,
    'dist-lambda',
    path.basename(input, path.extname(input))
  );
  await fs.rm(outdir, { recursive: true, force: true });
  for (const code of codes) {
    const outPath = path.join(outdir, code.fileName);
    await fs.mkdir(path.dirname(outPath), { recursive: true });
    await fs.writeFile(outPath, code.content);
  }
}
