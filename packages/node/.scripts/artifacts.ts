import fs from 'node:fs/promises';
import path from 'node:path';
import { NapiCli } from '@napi-rs/cli';
import { glob } from 'tinyglobby';

function resolveNodeFilePath(filePath: string): string {
  const dirname = path.dirname(filePath);
  const filename = path.basename(filePath);

  const [name, arch, ext] = filename.split('.');
  switch (`${arch}.${ext}`) {
    case 'darwin-x64.node':
      return path.join(dirname, `${name}-x86_64-apple-darwin`, filename);
    case 'darwin-arm64.node':
      return path.join(dirname, `${name}-aarch64-apple-darwin`, filename);
    case 'win32-ia32-msvc.node':
      return path.join(dirname, `${name}-i686-pc-windows-msvc`, filename);
    case 'win32-x64-msvc.node':
      return path.join(dirname, `${name}-x86_64-pc-windows-msvc`, filename);
    case 'win32-arm64-msvc.node':
      return path.join(dirname, `${name}-aarch64-pc-windows-msvc`, filename);
    case 'linux-x64-musl.node':
      return path.join(dirname, `${name}-x86_64-unknown-linux-musl`, filename);
    case 'linux-x64-gnu.node':
      return path.join(dirname, `${name}-x86_64-unknown-linux-gnu`, filename);
    case 'linux-arm64-musl.node':
      return path.join(dirname, `${name}-aarch64-unknown-linux-musl`, filename);
    case 'linux-arm64-gnu.node':
      return path.join(dirname, `${name}-aarch64-unknown-linux-gnu`, filename);
    case 'linux-arm-gnueabihf.node':
      return path.join(dirname, `${name}-armv7-unknown-linux-gnueabihf`, filename);
    case 'android-arm64.node':
      return path.join(dirname, `${name}-aarch64-linux-android`, filename);
    case 'android-arm-eabi.node':
      return path.join(dirname, `${name}-armv7-linux-androideabi`, filename);
    default:
      throw new Error(`unknown file: ${filename}`);
  }
}

const pkgDir = path.join(import.meta.dirname, '..');
const nodeFiles = await glob('*.node', {
  cwd: path.join(pkgDir, 'artifacts'),
  onlyFiles: true,
});

for (let i = 0; i < nodeFiles.length; i += 1) {
  const progress = `[${i + 1}/${nodeFiles.length}]`;
  const nodeFile = nodeFiles[i]!;

  const src = path.join(pkgDir, 'artifacts', nodeFile);
  const dest = resolveNodeFilePath(src);

  await fs.mkdir(path.dirname(dest), { recursive: true });
  await fs.copyFile(src, dest);

  console.log(`${progress} ${path.relative(pkgDir, dest)}: file copied`);
}

const cli = new NapiCli();
await cli.artifacts({ cwd: pkgDir });
