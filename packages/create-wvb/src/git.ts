import { spawn } from 'node:child_process';
import fs from 'node:fs/promises';
import path from 'node:path';

function git(args: readonly string[], cwd: string): Promise<boolean> {
  return new Promise(resolve => {
    const child = spawn('git', [...args], { cwd, stdio: 'ignore' });
    child.on('error', () => resolve(false));
    child.on('close', code => resolve(code === 0));
  });
}

async function isInsideRepository(dir: string): Promise<boolean> {
  return git(['rev-parse', '--is-inside-work-tree'], dir);
}

/**
 * Returns false when the repository was not created — already inside one, or git is unavailable.
 * Scaffolding must not fail because of git.
 */
export async function initRepository(dir: string): Promise<boolean> {
  if (await isInsideRepository(dir)) {
    return false;
  }
  if (!(await git(['init'], dir))) {
    return false;
  }
  if (!(await git(['add', '-A'], dir))) {
    await fs.rm(path.join(dir, '.git'), { recursive: true, force: true });
    return false;
  }
  const committed = await git(
    ['commit', '-m', 'Initial commit from create-wvb', '--no-verify'],
    dir
  );
  if (!committed) {
    await fs.rm(path.join(dir, '.git'), { recursive: true, force: true });
    return false;
  }
  return true;
}
