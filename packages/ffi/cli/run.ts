import { execa } from 'execa';

export interface RunCommandOptions {
  cwd?: string;
  env?: Record<string, string>;
  prefix?: string;
}

export async function runCommand(cmd: string, args: string[], options: RunCommandOptions = {}) {
  const { cwd, env, prefix = '' } = options;
  const stdout = function* (line: string) {
    yield `${prefix}${line}`;
  };
  const stderr = function* (line: string) {
    yield `${prefix}${line}`;
  };
  await (execa as any)(cmd, args, {
    cwd,
    stdout: [stdout, 'inherit'],
    stderr: [stderr, 'inherit'],
    reject: true,
    env,
  });
}
