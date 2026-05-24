import { execa } from 'execa';

export interface RunCommandOptions {
  cwd?: string;
  env?: Record<string, string>;
  prefix?: string;
  /** @default false */
  reject?: boolean;
}

export async function runCommand(cmd: string, args: string[], options: RunCommandOptions = {}) {
  const { cwd, env, prefix = '', reject = false } = options;
  const stdout = function* (line: string) {
    yield `${prefix}${line}`;
  };
  const stderr = function* (line: string) {
    yield `${prefix}${line}`;
  };
  const { exitCode } = await (execa as any)(cmd, args, {
    cwd,
    stdout: [stdout, 'inherit'],
    stderr: [stderr, 'inherit'],
    reject: false,
    env,
  });
  if (reject && exitCode !== 0) {
    throw new Error(`Command failed with exit code ${exitCode}`);
  }
  return { exitCode } as { exitCode: number | undefined };
}
