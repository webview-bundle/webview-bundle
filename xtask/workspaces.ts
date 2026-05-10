import fs from 'node:fs/promises';
import path from 'node:path';
import glob from 'fast-glob';
import micromatch from 'micromatch';
import type { PackageJson } from 'type-fest';
import { runCommand } from './child_process.ts';
import { colors } from './console.ts';
import { ROOT_DIR } from './consts.ts';

export interface Workspace {
  pkg: PackageJson;
  name: string;
  path: string;
  absolutePath: string;
}

let workspaces: Workspace[] | null = null;

export async function loadAllWorkspaces(): Promise<Workspace[]> {
  if (workspaces != null) {
    return workspaces;
  }
  const packageJsons = await glob('**/package.json', {
    cwd: ROOT_DIR,
    dot: false,
    onlyFiles: true,
    ignore: ['**/node_modules', '**/.yarn', '**/target', '**/dist', '**/esm'],
  });
  const allWorkspaces: Workspace[] = [];
  for (const pkgFilepath of packageJsons) {
    const filepath = path.join(ROOT_DIR, pkgFilepath);
    const pkgRaw = await fs.readFile(filepath, 'utf8');
    const pkg: PackageJson = JSON.parse(pkgRaw);
    const workspace: Workspace = {
      pkg,
      name: pkg.name!,
      path: path.dirname(pkgFilepath),
      absolutePath: path.dirname(filepath),
    };
    allWorkspaces.push(workspace);
  }
  workspaces = allWorkspaces;
  return workspaces;
}

export function matchWorkspaceByPattern(workspace: Workspace, pattern: string | string[]): boolean {
  const patterns = Array.isArray(pattern) ? pattern : [pattern];
  if (patterns.length === 0) {
    return false;
  }
  return patterns.some(x => {
    return micromatch.isMatch(workspace.name, x) || micromatch.isMatch(workspace.path, x);
  });
}

export function isRootWorkspace(workspace: Workspace): boolean {
  return workspace.path === '.';
}

export async function runWorkspaceCommand(workspace: Workspace, cmd: string, args: string[] = []) {
  const prefix = colors.info(`[${workspace.path}]`);
  console.log(`${prefix} run command: ${cmd} ${args.join()}`);
  return await runCommand(cmd, args, {
    cwd: workspace.absolutePath,
    prefix: `${prefix} `,
  });
}

export async function runWorkspaceScript(workspace: Workspace, script: string) {
  const prefix = colors.info(`[${workspace.path}]`);
  if (workspace.pkg.scripts?.[script] == null) {
    console.warn(`${prefix} no script found: ${script}`);
    return null;
  }
  return await runWorkspaceCommand(workspace, 'yarn', ['run', script]);
}
