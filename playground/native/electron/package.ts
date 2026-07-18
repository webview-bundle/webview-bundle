import { execFile } from 'node:child_process';
import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';
import packager from '@electron/packager';

const execFileAsync = promisify(execFile);

const root = import.meta.dirname;
const workDir = path.join(root, '.package');
const stageDir = path.join(workDir, 'stage');

await fs.rm(workDir, { recursive: true, force: true });
await fs.mkdir(stageDir, { recursive: true });
await fs.cp(path.join(root, 'dist'), path.join(stageDir, 'dist'), { recursive: true });

const pkgJson = JSON.parse(await fs.readFile(path.join(root, 'package.json'), 'utf8'));
await fs.writeFile(
  path.join(stageDir, 'package.json'),
  JSON.stringify(
    {
      name: pkgJson.name,
      productName: 'WebviewBundlePlaygroundElectron',
      version: pkgJson.version ?? '0.0.0',
      main: pkgJson.main,
    },
    null,
    2
  ),
  'utf8'
);

const require = createRequire(root);

// Copy native modules
const wvbNodeDir = path.dirname(require.resolve('@wvb/node/package.json'));
const wvbNodeDest = path.join(stageDir, 'node_modules', '@wvb', 'node');
await fs.mkdir(wvbNodeDest, { recursive: true });
await fs.cp(wvbNodeDir, wvbNodeDest, { recursive: true });

const [appPath] = await packager({
  dir: stageDir,
  out: path.join(root, 'out'),
  overwrite: true,
  electronVersion: require('electron/package.json').version,
  asar: {
    unpack: '**/*.node',
    unpackDir: '**/@wvb/node',
  },
  extraResource: [path.join(root, '.wvb', 'builtin', 'bundles')],
  prune: false,
});
if (appPath == null) {
  throw new Error('packager produced no output directory');
}

console.log(`Packaged: ${appPath}`);

if (process.platform === 'darwin') {
  const appName = (await fs.readdir(appPath)).find(name => name.endsWith('.app'));
  if (appName == null) {
    throw new Error(`no .app found in ${appPath}`);
  }

  const dmgSrc = path.join(workDir, 'dmg');
  await fs.rm(dmgSrc, { recursive: true, force: true });
  await fs.mkdir(dmgSrc, { recursive: true });
  await fs.cp(path.join(appPath, appName), path.join(dmgSrc, appName), {
    recursive: true,
    verbatimSymlinks: true,
  });
  await fs.symlink('/Applications', path.join(dmgSrc, 'Applications'));

  const dmgPath = path.join(root, 'out', 'WebviewBundlePlaygroundElectron.dmg');
  await fs.rm(dmgPath, { force: true });
  await execFileAsync('hdiutil', [
    'create',
    '-volname',
    'WebviewBundlePlaygroundElectron',
    '-srcfolder',
    dmgSrc,
    '-format',
    'UDZO',
    '-ov',
    dmgPath,
  ]);
  console.log(`DMG: ${dmgPath}`);
}
