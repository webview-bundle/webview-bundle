import { execSync } from 'node:child_process';
import fs from 'node:fs';
import { EOL } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import pkgJson from './package.js' with { type: 'json' };

const dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(dirname, '..', '..');

const { dependencies = {} } = pkgJson;

const result = execSync('yarn workspaces list --json', {
  cwd: rootDir,
  encoding: 'utf8',
});
const workspaces = result
  .split(EOL)
  .filter(x => x.trim().length > 0)
  .map(x => {
    try {
      return JSON.parse(x);
    } catch {
      return null;
    }
  })
  .filter(x => x != null);

for (const [name, version] of Object.entries(dependencies)) {
  if (!name.startsWith('@wvb/')) {
    continue;
  }
  const expectFileName = `wvb-${name.split('/')[1]}.tgz`;
  const expectVersion = `file:./${expectFileName}`;
  if (version !== expectVersion) {
    throw new Error(
      `Expect "${name}" package version to be "${expectVersion}" (but it is "${version}")`
    );
  }
  const cmd = `yarn workspace ${name} pack`;
  console.log(`Run command: ${cmd}`);
  execSync(cmd, { cwd: rootDir, stdio: 'inherit' });

  const workspace = workspaces.find(x => x.name === name);
  if (workspace == null) {
    throw new Error(`Cannot find yarn workspace for package: "${name}"`);
  }
  const src = path.join(rootDir, workspace.location, 'package.tgz');
  const dest = path.join(dirname, expectFileName);
  console.log(`Moving package: ${path.relative(rootDir, src)} -> ${path.relative(rootDir, dest)}`);
  fs.copyFileSync(src, dest);
}

execSync('npm install', {
  cwd: dirname,
  stdio: 'inherit',
});
