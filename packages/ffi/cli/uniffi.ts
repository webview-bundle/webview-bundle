import path from 'node:path';
import { PKG_DIR, ROOT_DIR } from './consts.ts';
import { pathExists } from './fs.ts';
import { runCommand } from './run.ts';

export async function generateUniffiBindings(
  language: 'kotlin' | 'swift',
  libPath: string,
  outDir: string
): Promise<void> {
  if (!(await pathExists(libPath))) {
    throw new Error(`Library not found: ${libPath}`);
  }

  const args = [
    'uniffi-bindgen',
    'generate',
    libPath,
    '--language',
    language,
    '--out-dir',
    outDir,
    '--no-format',
    '--config',
    path.join(PKG_DIR, 'uniffi.toml'),
  ];

  await runCommand('cargo', args, {
    cwd: ROOT_DIR,
    prefix: `[uniffi:${language}] `,
  });
}
