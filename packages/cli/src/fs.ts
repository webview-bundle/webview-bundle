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

const WINDOWS_RESERVED_NAMES = new Set([
  'CON',
  'PRN',
  'AUX',
  'NUL',
  'COM1',
  'COM2',
  'COM3',
  'COM4',
  'COM5',
  'COM6',
  'COM7',
  'COM8',
  'COM9',
  'LPT1',
  'LPT2',
  'LPT3',
  'LPT4',
  'LPT5',
  'LPT6',
  'LPT7',
  'LPT8',
  'LPT9',
]);

/**
 * Whether `value` is safe to use verbatim as a single filesystem path component on
 * Windows, macOS, and Linux. Mirrors `is_valid_path_component` in the core
 * (`packages/core/src/source/source.rs`), which rejects bundle names/versions that
 * fail this check when resolving builtin/remote bundle filepaths.
 */
export function isValidPathComponent(value: string): boolean {
  if (value.length === 0 || value === '.' || value === '..') {
    return false;
  }
  if (!/^[A-Za-z0-9._-]+$/.test(value)) {
    return false;
  }
  const base = value.split('.')[0] ?? value;
  return !WINDOWS_RESERVED_NAMES.has(base.toUpperCase());
}
