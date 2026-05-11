import fs from 'node:fs/promises';
import path from 'node:path';

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

export interface PackageJson {
  name?: string;
  type?: 'module' | 'commonjs';
  version?: string;
  description?: string;
  peerDependencies?: Record<string, string>;
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
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

export async function findNearestPackageJson(basedir: string): Promise<PackageJson | null> {
  const pkgJsonPath = await findNearestPackageJsonFilePath(basedir);
  if (pkgJsonPath == null) {
    return null;
  }
  const raw = await fs.readFile(pkgJsonPath, 'utf8');
  const json = JSON.parse(raw);
  return json as PackageJson;
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

export function normalizeFileName(filename: string): string {
  return filename.replace(/\//g, '-');
}

export function withFileExtension(filename: string, ext: string): string {
  const currentExt = path.extname(filename);
  if (currentExt === ext) {
    return filename;
  }
  return `${filename}${ext}`;
}

export function withWVBExtension(filename: string): string {
  return withFileExtension(filename, '.wvb');
}
