import fs from 'node:fs/promises';
import { EOL } from 'node:os';
import path from 'node:path';
import { diffLines } from 'diff';
import { match } from 'ts-pattern';
import { runCommand } from './child_process.ts';
import { c } from './console.ts';
import { ROOT_DIR } from './consts.ts';

export type Action =
  | { type: 'write'; path: string; content: string; prevContent?: string }
  | { type: 'command'; cmd: string; args: string[]; path: string };

export function formatAction(action: Action): string {
  switch (action.type) {
    case 'write':
      return ['write action', c.dim(`  path: ${action.path}`)].join(EOL);
    case 'command':
      return [
        'command action',
        c.dim(`  path: ${action.path}`),
        c.dim(`  cmd: ${action.cmd}`),
        c.dim(`  args: ${action.args.join(' ')}`),
      ].join(EOL);
  }
}

export interface RunActionsContext {
  name?: string;
  dryRun?: boolean;
  failFast?: boolean;
  reject?: boolean;
}

export type RunActionResult =
  | { succeed: true; action: Action }
  | { succeed: false; action: Action; error: Error };
export type RunActionsResult =
  | {
      allSucceed: true;
      items: Array<Extract<RunActionResult, { succeed: true }>>;
      ctx: Required<RunActionsContext>;
    }
  | { allSucceed: false; items: RunActionResult[]; ctx: Required<RunActionsContext> };

export async function runActions(
  actions: Action[],
  initialCtx: RunActionsContext = {}
): Promise<RunActionsResult> {
  const { name = 'root', dryRun = false, failFast = true, reject = true } = initialCtx;
  const ctx: Required<RunActionsContext> = { name, dryRun, failFast, reject };
  if (dryRun) {
    dryRunActions(name, actions);
    return {
      allSucceed: true,
      items: actions.map(action => ({ succeed: true, action })),
      ctx,
    };
  }
  const items: RunActionResult[] = [];
  const rejectFailures = (): never => {
    const failureCount = items.filter(x => !x.succeed).length;
    throw new Error(`${c.error(`[${name}]`)} ${failureCount} action(s) failed`);
  };
  for (const action of actions) {
    const item = await match(action)
      .with({ type: 'write' }, x => runWriteAction(name, x))
      .with({ type: 'command' }, x => runCommandAction(name, x))
      .exhaustive();
    items.push(item);
    if (failFast && !item.succeed) {
      if (reject) {
        rejectFailures();
      }
      return { allSucceed: false, items, ctx };
    }
  }
  const result = {
    allSucceed: items.every(x => x.succeed),
    items,
    ctx,
  } as RunActionsResult;
  if (reject && !result.allSucceed) {
    rejectFailures();
  }
  return result;
}

async function runWriteAction(
  name: string,
  action: Extract<Action, { type: 'write' }>
): Promise<RunActionResult> {
  console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
  try {
    const filepath = path.join(ROOT_DIR, action.path);
    await fs.mkdir(path.dirname(filepath), { recursive: true });
    await fs.writeFile(filepath, action.content, 'utf8');
    if (action.prevContent != null) {
      logDiff(action.prevContent, action.content);
    }
    return { succeed: true, action };
  } catch (e) {
    const error = e as Error;
    console.error(`${c.error(`[${name}]`)} write command failed: ${error.message}`);
    return { succeed: false, action, error };
  }
}

async function runCommandAction(
  name: string,
  action: Extract<Action, { type: 'command' }>
): Promise<RunActionResult> {
  console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
  const { exitCode } = await runCommand(action.cmd, action.args, {
    cwd: path.join(ROOT_DIR, action.path),
    prefix: `${c.info(`[${name}]`)} `,
  });
  if (exitCode !== 0) {
    console.error(`${c.error(`[${name}]`)} command action failed: exitCode=${exitCode}`);
    return {
      succeed: false,
      action,
      error: new Error(`command failed with exitCode: ${exitCode}`),
    };
  }
  return { succeed: true, action };
}

function dryRunActions(name: string, actions: Action[]) {
  for (const action of actions) {
    console.log(`${c.info(`[${name}]`)} ${formatAction(action)}`);
    if (action.type === 'write' && action.prevContent != null) {
      logDiff(action.prevContent, action.content);
    }
  }
}

function logDiff(prevContent: string, content: string) {
  const diff = diffLines(prevContent, content);
  let modified: number | undefined;
  let lineNo = 0;
  for (const change of diff) {
    if (!change.added && !change.removed) {
      if (modified != null) {
        lineNo += modified;
        modified = undefined;
      }
      lineNo += change.count ?? 0;
      continue;
    }
    let changeLineNo = lineNo;
    const lines = change.value.trimEnd().split('\n');
    for (const line of lines) {
      changeLineNo += 1;
      const lineStr = String(changeLineNo).padStart(3, ' ');
      const color = (str: string | number): string => {
        if (change.added) {
          return c.success(str);
        }
        return c.error(str);
      };
      const diffPrefix = change.added ? '+' : '-';
      console.log(`  ${c.dim(lineStr)}|${diffPrefix}${color(line)}`);
    }
    modified = change.count;
  }
}
