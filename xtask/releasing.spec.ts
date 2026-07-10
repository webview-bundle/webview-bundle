import { describe, expect, it, vi } from 'vitest';
import type { PackageConfig } from './config.ts';
import { Package } from './package.ts';
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

function crateManifest(
  name: string,
  version: string,
  opts: { publish?: boolean } = {}
): VersionedFile {
  const publishLine = opts.publish === false ? '\npublish = false' : '';
  const file = VersionedFile.parse(
    'Cargo.toml',
    `packages/test/${name}/Cargo.toml`,
    `[package]\nname = "${name}"\nversion = "${version}"${publishLine}\n`
  );
  if (file == null) {
    throw new Error('failed to build manifest fixture');
  }
  return file;
}

function denoManifest(
  name: string,
  version: string,
  extra: Record<string, unknown> = {}
): VersionedFile {
  // A directory that does not exist, so the source-import scan is a no-op in these unit tests.
  const file = VersionedFile.parse(
    'deno.json',
    `packages/test/${name.replaceAll('/', '_')}/deno.json`,
    JSON.stringify({ name, version, ...extra })
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

describe('deno.json manifest', () => {
  it('parses a named, versioned deno.json as a jsr release target', () => {
    const file = denoManifest('@wvb/deno', '0.2.0');
    expect(file.name).toBe('@wvb/deno');
    expect(file.canPublish).toBe(true);
    expect(file.registry).toEqual({
      name: '@wvb/deno',
      type: 'jsr',
      version: '0.2.0',
      url: 'https://jsr.io/@wvb/deno@0.2.0',
    });
    expect(file.publishAction(file.version)).toMatchObject({
      type: 'publish',
      registry: 'jsr',
      manifest: '@wvb/deno',
      version: '0.2.0',
      cmd: 'deno',
      args: ['publish', '--allow-dirty'],
    });
  });

  it('is not a release target when versionless (workspace-root deno.json)', () => {
    const file = VersionedFile.parse(
      'deno.json',
      'packages/deno.json',
      JSON.stringify({ workspace: ['./deno'], imports: { '@std/path': 'jsr:@std/path@^1' } })
    );
    expect(file).toBeNull();
  });

  it('honors private: true', () => {
    expect(denoManifest('@wvb/deno', '0.2.0', { private: true }).canPublish).toBe(false);
  });

  it('derives dependency names from the imports map', () => {
    const file = denoManifest('@wvb/deno-desktop', '0.2.0', {
      imports: { '@wvb/deno': 'jsr:@wvb/deno@^0.2.0', '@std/path': 'jsr:@std/path@^1' },
    });
    // The source scan is empty (fixture dir does not exist), so only the imports keys remain.
    expect(file.dependencyNames).toEqual(['@wvb/deno', '@std/path']);
  });
});

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

  it('publishes a deno package via deno, checking the jsr registry by manifest type', async () => {
    // packages/deno ships a non-publishable cdylib crate alongside the JSR-published deno.json.
    const pkg = makePackage([
      crateManifest('wvb-deno', '0.2.0', { publish: false }),
      denoManifest('@wvb/deno', '0.2.0'),
    ]);
    const run = vi.fn(async (_cmd: string, _args: string[], _opts: { cwd: string }) => ({
      exitCode: 0,
      output: '',
    }));
    const exists = vi.fn(async () => false as boolean | null);
    const outcome = await publishPackage(pkg, {
      current: true,
      ports: makePorts({ proc: { run }, registry: { exists } }),
    });
    expect(outcome.status).toBe('published');
    expect(exists).toHaveBeenCalledWith('deno.json', '@wvb/deno', '0.2.0');
    expect(run).toHaveBeenCalledTimes(1);
    expect(run.mock.calls[0]![0]).toBe('deno');
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
