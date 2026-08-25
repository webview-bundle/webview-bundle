import { spawn } from 'node:child_process';

export const PACKAGE_MANAGERS = ['npm', 'yarn', 'pnpm', 'bun'] as const;
export type PackageManager = (typeof PACKAGE_MANAGERS)[number];

export function detectPackageManager(): PackageManager {
  // npm_config_user_agent looks like "yarn/4.17.1 npm/? node/v24.15.0 darwin arm64".
  const agent = process.env.npm_config_user_agent?.split(' ')[0]?.split('/')[0];
  if (agent != null && (PACKAGE_MANAGERS as readonly string[]).includes(agent)) {
    return agent as PackageManager;
  }
  if (process.versions.bun != null) {
    return 'bun';
  }
  return 'npm';
}

export function runScript(pm: PackageManager, script: string): string {
  return pm === 'npm' ? `npm run ${script}` : `${pm} ${script}`;
}

export function runPrefix(pm: PackageManager): string {
  return pm === 'npm' ? 'npm run' : pm;
}

function exec(
  command: string,
  args: readonly string[],
  cwd: string,
  env: NodeJS.ProcessEnv
): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, [...args], {
      cwd,
      stdio: 'ignore',
      shell: process.platform === 'win32',
      env,
    });
    child.on('error', reject);
    child.on('close', code => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`"${command} ${args.join(' ')}" exited with code ${code}.`));
    });
  });
}

/** Bun has no offline flag (only `--frozen-lockfile` / `--no-cache`), so `--offline` cannot be honored there. */
export function supportsOffline(pm: PackageManager): boolean {
  return pm !== 'bun';
}

export async function install(
  pm: PackageManager,
  cwd: string,
  options: { readonly offline?: boolean } = {}
): Promise<void> {
  const env = { ...process.env };
  const args = ['install'];
  if (options.offline === true) {
    switch (pm) {
      case 'yarn':
        // Yarn 4 dropped `--offline`; disabling the network for the run is the equivalent.
        env.YARN_ENABLE_NETWORK = 'false';
        break;
      case 'npm':
      case 'pnpm':
        args.push('--offline');
        break;
      case 'bun':
        break;
    }
  }
  await exec(pm, args, cwd, env);
}
