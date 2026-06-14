import fs from 'node:fs/promises';
import path from 'node:path';
import type { PackageJson } from '@wvb/config';

export async function isEsmFile(filepath: string): Promise<boolean> {
  if (/\.m[jt]s$/.test(filepath)) {
    return true;
  }
  if (/\.c[jt]s$/.test(filepath)) {
    return false;
  }
  try {
    const pkg = await findNearestPackageJson(path.dirname(filepath));
    return pkg?.type === 'module';
  } catch {
    return false;
  }
}

export async function findNearestPackageJsonFilePath(basedir: string): Promise<string | null> {
  let dir = basedir;
  while (dir) {
    const pkgJsonPath = path.join(dir, 'package.json');
    const stat = await fs.stat(pkgJsonPath).catch(() => null);
    if (stat?.isFile() === true) {
      return pkgJsonPath;
    }
    const nextDir = path.dirname(dir);
    if (nextDir === dir) {
      break;
    }
    dir = nextDir;
  }
  return null;
}

export async function findNearestPackageJson(basedir: string): Promise<PackageJson | undefined> {
  const pkgJsonPath = await findNearestPackageJsonFilePath(basedir);
  if (pkgJsonPath == null) {
    return undefined;
  }
  const raw = await fs.readFile(pkgJsonPath, 'utf8');
  try {
    const json = JSON.parse(raw);
    return json as PackageJson;
  } catch {
    throw new Error('Fail to parse "package.json"');
  }
}

export async function pathExists(p: string): Promise<boolean> {
  try {
    await fs.access(p);
    return true;
  } catch {
    return false;
  }
}

export function toAbsolutePath(p: string, cwd: string): string {
  return path.isAbsolute(p) ? p : path.join(cwd, p);
}

export function withFileExtension(filename: string, ext: string): string {
  const currentExt = path.extname(filename);
  if (currentExt === ext) {
    return filename;
  }
  return `${filename}${ext}`;
}

export function withWvbExtension(filename: string): string {
  return withFileExtension(filename, '.wvb');
}
