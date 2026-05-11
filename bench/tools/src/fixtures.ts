import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execa } from 'execa';
import { Listr } from 'listr2';
import { glob } from 'tinyglobby';

const dirname = path.dirname(fileURLToPath(import.meta.url));
const current = path.resolve(dirname, '..');
const fixturesDir = path.resolve(current, '..', 'fixtures');

export interface FixtureInfo {
  name: string;
  dir: string;
}

export function listFixtures(): FixtureInfo[] {
  const names = fs.readdirSync(fixturesDir);
  const list: FixtureInfo[] = [];
  for (const name of names) {
    const dir = path.join(fixturesDir, name);
    const stat = fs.statSync(dir);
    if (stat.isDirectory()) {
      try {
        fs.accessSync(path.join(dir, 'package.json'));
        list.push({ name, dir });
      } catch {}
    }
  }
  return list;
}

export function getFixtureDir(name: string): string {
  return path.join(fixturesDir, name);
}

export function getFixtureOutDir(name: string): string {
  return path.join(getFixtureDir(name), 'out');
}

export function getFixtureOutWebviewBundleFilePath(name: string): string {
  return path.join(fixturesDir, 'bundles', name, `${name}_1.0.0.wvb`);
}

export interface FixtureFile {
  path: string;
  absolutePath: string;
}

export async function findAllFixtureFiles(name: string): Promise<FixtureFile[]> {
  const outDir = getFixtureOutDir(name);
  const paths = await glob('**/*', {
    cwd: getFixtureOutDir(name),
    dot: true,
    absolute: false,
    onlyFiles: true,
  });
  const files = paths.map((filePath): FixtureFile => {
    return {
      path: filePath,
      absolutePath: path.join(outDir, filePath),
    };
  });
  return files;
}

let cliPrepared = false;

export async function prepareCli(name: string): Promise<void> {
  if (cliPrepared) {
    return;
  }
  const require = createRequire(getFixtureDir(name));
  const cliPath = path.dirname(require.resolve('@wvb/cli/package.json'));
  await execa('yarn', ['build'], { cwd: cliPath });
  cliPrepared = true;
}

export async function buildFixture(name: string): Promise<boolean> {
  await execa('yarn', ['out'], { cwd: getFixtureDir(name) });
  await prepareCli(name);
  await execa(
    'yarn',
    [
      'wvb',
      'create',
      getFixtureOutDir(name),
      '-O',
      getFixtureOutWebviewBundleFilePath(name),
      '--truncate',
    ],
    { cwd: current }
  );
  return true;
}

export async function buildAllFixtures(): Promise<void> {
  const fixtures = listFixtures();
  const task = new Listr(
    fixtures.map(fixture => ({
      title: `Building "${fixture.name}"...`,
      task: async () => {
        await buildFixture(fixture.name);
      },
    }))
  );
  await task.run();
}
