import fs from 'node:fs/promises';
import path from 'node:path';

const VALID_NAME = /^(?:@[a-z0-9-~][a-z0-9-._~]*\/)?[a-z0-9-~][a-z0-9-._~]*$/;

export function validateProjectName(name: string): string | null {
  if (name.length === 0) {
    return 'Project name is required.';
  }
  if (name.length > 214) {
    return 'Project name must be 214 characters or fewer.';
  }
  if (!VALID_NAME.test(name)) {
    return 'Project name must be lowercase and url-safe (letters, digits, "-", "_", "."), optionally scoped.';
  }
  return null;
}

export function toProjectName(target: string): string {
  const base = path.basename(path.resolve(target));
  const name = base
    .toLowerCase()
    .replace(/[^a-z0-9-._~]+/g, '-')
    .replace(/^[-._]+|[-._]+$/g, '');
  return name === '' ? 'my-wvb-app' : name;
}

/**
 * The bundle name is the web workspace's package name and the host of every `<scheme>://<bundle>.wvb`
 * URL. The default hostname resolver splits on `.` and takes the first label, so a dot in the name
 * routes `a.b.wvb` to bundle `a`; the mobile route matcher also rejects `~` and a leading `-`. The
 * name is therefore reduced to a host-safe identifier — the CLI strips only the scope, so this stays
 * the identity the packed bundle is stored under.
 */
export function toBundleName(projectName: string): string {
  const unscoped = projectName.startsWith('@')
    ? (projectName.split('/')[1] ?? projectName)
    : projectName;
  const safe = unscoped
    .toLowerCase()
    .replace(/[^a-z0-9-]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return safe === '' ? 'app' : safe;
}

export interface TargetState {
  readonly exists: boolean;
  readonly conflicts: readonly string[];
}

export async function inspectTarget(dir: string): Promise<TargetState> {
  let entries: string[];
  try {
    entries = await fs.readdir(dir);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return { exists: false, conflicts: [] };
    }
    throw error;
  }
  const conflicts = entries.filter(entry => entry !== '.git' && entry !== '.DS_Store');
  return { exists: true, conflicts };
}
