import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import micromatch from 'micromatch';
import pAll from 'p-all';
import { glob } from 'tinyglobby';
import * as t from 'typanion';
import type { PackageJson } from 'type-fest';
import { runCommand } from '../child_process.ts';
import { ColorModeOption, colors, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';

interface Workspace {
  pkg: PackageJson;
  name: string;
  path: string;
  absolutePath: string;
}

async function loadAllWorkspaces(): Promise<Workspace[]> {
  const packageJsons = await glob('**/package.json', {
    cwd: ROOT_DIR,
    dot: false,
    onlyFiles: true,
    ignore: ['**/node_modules', '**/.yarn', '**/target', '**/dist', '**/esm'],
  });
  const workspaces: Workspace[] = [];
  for (const pkgFilepath of packageJsons) {
    const filepath = path.join(ROOT_DIR, pkgFilepath);
    const pkgRaw = await fs.readFile(filepath, 'utf8');
    const pkg: PackageJson = JSON.parse(pkgRaw);
    workspaces.push({
      pkg,
      name: pkg.name!,
      path: path.dirname(pkgFilepath),
      absolutePath: path.dirname(filepath),
    });
  }
  return workspaces;
}

function matchWorkspaceByPattern(workspace: Workspace, pattern: string | string[]): boolean {
  const patterns = Array.isArray(pattern) ? pattern : [pattern];
  if (patterns.length === 0) {
    return false;
  }
  return patterns.some(x => {
    return micromatch.isMatch(workspace.name, x) || micromatch.isMatch(workspace.path, x);
  });
}

function isRootWorkspace(workspace: Workspace): boolean {
  return workspace.path === '.';
}

async function runWorkspaceCommand(workspace: Workspace, cmd: string, args: string[] = []) {
  const prefix = colors.info(`[${workspace.path}]`);
  console.log(`${prefix} run command: ${cmd} ${args.join()}`);
  return await runCommand(cmd, args, {
    cwd: workspace.absolutePath,
    prefix: `${prefix} `,
  });
}

export class AttwCommand extends Command {
  static paths = [['attw']];

  readonly include = Option.Array('--include', {
    description: 'Patterns to include workspaces',
    required: false,
  });
  readonly exclude = Option.Array('--exclude', {
    description: 'Patterns to include workspaces',
    required: false,
  });
  readonly concurrency = Option.String('--concurrency,-C', {
    description: 'Concurrency count. Default is 3',
    required: false,
    validator: t.cascade(t.isNumber(), [t.isInteger()]),
  });
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    const allWorkspaces = await loadAllWorkspaces();
    const includePattern = this.include ?? [];
    const excludePattern = this.exclude ?? [];
    const workspaces = allWorkspaces
      .filter(x => (includePattern.length > 0 ? matchWorkspaceByPattern(x, includePattern) : true))
      .filter(x => (excludePattern.length > 0 ? !matchWorkspaceByPattern(x, excludePattern) : true))
      .filter(x => !isRootWorkspace(x));
    if (workspaces.length === 0) {
      console.warn('no workspaces matched');
      return 0;
    }
    const concurrency = Math.max(this.concurrency ?? 3, 1);
    const results = await pAll(
      workspaces.map(workspace => async () => {
        const prefix = colors.info(`[${workspace.path}]`);
        if (workspace.pkg.private === true) {
          console.warn(`${prefix} workspace is private. skip.`);
          return { type: 'skipped' as const };
        }
        const packed = await runWorkspaceCommand(workspace, 'yarn', ['pack']);
        if (packed.exitCode !== 0) {
          return {
            type: 'failed' as const,
            error: `${prefix} pack failed with exit code: ${packed.exitCode}`,
          };
        }
        const attw = await runCommand(
          'yarn',
          ['exec', 'attw', path.join(workspace.absolutePath, 'package.tgz'), '--profile', 'node16'],
          {
            cwd: ROOT_DIR,
            prefix: `${prefix} `,
          }
        );
        if (attw.exitCode !== 0) {
          return {
            type: 'failed' as const,
            error: `${prefix} attw failed with exit code: ${attw.exitCode}`,
          };
        }
        return { type: 'succeed' as const };
      }),
      { concurrency }
    );
    let exitCode = 0;
    for (const result of results) {
      if (result.type === 'failed') {
        exitCode = 1;
        console.error(result.error);
      }
    }
    return exitCode;
  }
}
