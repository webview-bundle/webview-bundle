import { describe, expect, it, vi } from 'vitest';
import { Package } from './package.ts';
import type { PackageConfig } from './package-config.ts';
import type { Ports } from './ports.ts';
import { observePublishState, planPackagePublish, publishPackage } from './releasing.ts';
import { VersionedFile } from './versioned-file.ts';

function npmManifest(
  name: string,
  version: string,
  opts: { private?: boolean } = {}
): VersionedFile {
  const file = VersionedFile.parse(
    'package.json',
    `packages/test/${name.replaceAll('/', '_')}/package.json`,
    JSON.stringify({ name, version, private: opts.private })
  );
  if (file == null) {
    throw new Error('failed to build manifest fixture');
  }
  return file;
}

function crateManifest(name: string, version: string): VersionedFile {
  const file = VersionedFile.parse(
    'Cargo.toml',
    `packages/test/${name}/Cargo.toml`,
    `[package]\nname = "${name}"\nversion = "${version}"\n`
  );
  if (file == null) {
    throw new Error('failed to build manifest fixture');
  }
  return file;
}

function makePackage(files: VersionedFile[], config: PackageConfig = {}): Package {
  return new Package('test', 'packages/test', [files[0]!, ...files.slice(1)], config);
}

function makePorts(overrides: Partial<Ports> = {}): Ports {
  return {
    proc: { run: vi.fn(async () => ({ exitCode: 0, output: '' })) },
    registry: { exists: vi.fn(async () => false) },
    git: null,
    github: null,
    ...overrides,
  };
}

describe('observePublishState', () => {
  it('checks the current versions of every publishable manifest', async () => {
    const pkg = makePackage([
      npmManifest('@wvb/node', '0.2.0'),
      crateManifest('wvb', '0.2.0'),
      npmManifest('@wvb/private', '0.2.0', { private: true }),
    ]);
    const exists = vi.fn(async () => true as boolean | null);
    const states = await observePublishState(pkg, {
      current: true,
      ports: makePorts({ registry: { exists } }),
    });
    expect(states.map(s => [s.file.name, s.exists])).toEqual([
      ['@wvb/node', true],
      ['wvb', true],
    ]);
    expect(exists).toHaveBeenCalledWith('package.json', '@wvb/node', '0.2.0');
    expect(exists).toHaveBeenCalledWith('Cargo.toml', 'wvb', '0.2.0');
  });

  it('only observes bumped manifests when publishing pending versions', async () => {
    const bumped = npmManifest('@wvb/node', '0.2.0');
    bumped.setPrerelease('next', 'abc1234');
    const untouched = crateManifest('wvb', '0.2.0');
    const pkg = makePackage([bumped, untouched]);
    const exists = vi.fn(async () => false as boolean | null);
    const states = await observePublishState(pkg, { ports: makePorts({ registry: { exists } }) });
    expect(states.map(s => [s.file.name, s.version.toString()])).toEqual([
      ['@wvb/node', '0.2.0-next.abc1234'],
    ]);
  });

  it('stays offline on dry runs', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')]);
    const exists = vi.fn(async () => true as boolean | null);
    const states = await observePublishState(pkg, {
      current: true,
      dryRun: true,
      ports: makePorts({ registry: { exists } }),
    });
    expect(exists).not.toHaveBeenCalled();
    expect(states[0]?.exists).toBe(false);
  });
});

describe('planPackagePublish', () => {
  const scriptsConfig: PackageConfig = {
    beforePublishScripts: [{ command: 'yarn', args: ['artifacts'] }],
  };

  it('plans nothing when every version is already live', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')], scriptsConfig);
    const manifests = await observePublishState(pkg, {
      current: true,
      ports: makePorts({ registry: { exists: async () => true } }),
    });
    const plan = planPackagePublish(pkg, manifests);
    expect(plan).toMatchObject({ alreadyPublished: true, scripts: [], publishes: [] });
  });

  it('plans only the missing publishes, with the beforePublish scripts', async () => {
    const pkg = makePackage(
      [npmManifest('@wvb/node', '0.2.0'), crateManifest('wvb', '0.2.0')],
      scriptsConfig
    );
    const manifests = await observePublishState(pkg, {
      current: true,
      ports: makePorts({
        registry: { exists: async (_type, name) => name === '@wvb/node' },
      }),
    });
    const plan = planPackagePublish(pkg, manifests);
    expect(plan.alreadyPublished).toBe(false);
    expect(plan.scripts).toHaveLength(1);
    expect(plan.publishes).toMatchObject([
      { type: 'publish', registry: 'cargo', manifest: 'wvb', version: '0.2.0' },
    ]);
  });

  it('treats an unanswerable registry check as pending', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')]);
    const manifests = await observePublishState(pkg, {
      current: true,
      ports: makePorts({ registry: { exists: async () => null } }),
    });
    const plan = planPackagePublish(pkg, manifests);
    expect(plan.publishes).toHaveLength(1);
  });

  it('plans nothing for a package without publishable manifests', async () => {
    const pkg = makePackage([npmManifest('@wvb/private', '0.2.0', { private: true })]);
    const manifests = await observePublishState(pkg, { current: true, ports: makePorts() });
    const plan = planPackagePublish(pkg, manifests);
    expect(plan).toMatchObject({ alreadyPublished: false, scripts: [], publishes: [] });
  });
});

describe('publishPackage (observe → plan → apply)', () => {
  it('publishes what is missing and reports the package published', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0'), crateManifest('wvb', '0.2.0')]);
    const run = vi.fn(async (_cmd: string, _args: string[], _opts: { cwd: string }) => ({
      exitCode: 0,
      output: '',
    }));
    const outcome = await publishPackage(pkg, {
      current: true,
      ports: makePorts({
        proc: { run },
        registry: { exists: async (_type, name) => name === '@wvb/node' },
      }),
    });
    expect(outcome.status).toBe('published');
    // Only the crate was missing; the npm manifest was skipped by the registry check.
    expect(run).toHaveBeenCalledTimes(1);
    expect(run.mock.calls[0]![0]).toBe('cargo');
  });

  it('reports already-published when every version is live', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')]);
    const outcome = await publishPackage(pkg, {
      current: true,
      ports: makePorts({ registry: { exists: async () => true } }),
    });
    expect(outcome.status).toBe('already-published');
  });

  it('counts a duplicate-version rejection as published (staged npm)', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')]);
    const outcome = await publishPackage(pkg, {
      current: true,
      ports: makePorts({
        proc: {
          run: async () => ({
            exitCode: 1,
            output: 'You cannot publish over the previously published versions: 0.2.0.',
          }),
        },
      }),
    });
    expect(outcome.status).toBe('published');
  });

  it('fails without publishing when the beforePublish scripts fail', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')], {
      beforePublishScripts: [{ command: 'yarn', args: ['artifacts'] }],
    });
    const run = vi.fn(async () => ({ exitCode: 1, output: 'build broke' }));
    const outcome = await publishPackage(pkg, {
      current: true,
      ports: makePorts({ proc: { run } }),
    });
    expect(outcome).toMatchObject({ status: 'failed', reason: 'beforePublish scripts failed' });
    // The failing script must stop the publish itself from running.
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('fails when a publish fails for any other reason', async () => {
    const pkg = makePackage([npmManifest('@wvb/node', '0.2.0')]);
    const outcome = await publishPackage(pkg, {
      current: true,
      ports: makePorts({
        proc: { run: async () => ({ exitCode: 1, output: 'E401 unauthorized' }) },
      }),
    });
    expect(outcome).toMatchObject({ status: 'failed', reason: 'publish failed' });
  });
});
