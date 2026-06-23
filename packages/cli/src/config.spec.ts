import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { loadConfigFile, resolveConfig } from './config.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-config-'));
  await write(
    'package.json',
    JSON.stringify({ name: '@scope/fixture-app', version: '1.2.3', type: 'module' })
  );
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

function write(rel: string, content: string) {
  return fs.writeFile(path.join(root, rel), content);
}

describe('resolveConfig', () => {
  it('returns the inline config with defaults filled when no config file exists', async () => {
    const resolved = await resolveConfig({ root, pack: { srcDir: './src' } });

    expect(resolved.root).toBe(root);
    expect(resolved.configFile).toBeUndefined();
    expect(resolved.configFileDependencies).toEqual([]);
    expect(resolved.pack?.srcDir).toBe('./src');
    // packageJson is resolved from the nearest package.json to `root`.
    expect(resolved.packageJson?.name).toBe('@scope/fixture-app');
    // The original inline object is echoed back verbatim.
    expect(resolved.inlineConfig).toEqual({ root, pack: { srcDir: './src' } });
  });

  it('auto-discovers and merges a config file', async () => {
    await write(
      'wvb.config.mjs',
      `export default { remote: { endpoint: 'https://from-file' }, builtin: { clean: false } };`
    );

    const resolved = await resolveConfig({ root });

    expect(resolved.remote?.endpoint).toBe('https://from-file');
    expect(resolved.builtin?.clean).toBe(false);
    expect(resolved.configFile).toBe(path.join(root, 'wvb.config.mjs'));
    expect(resolved.configFileDependencies?.length).toBeGreaterThan(0);
  });

  it('lets inline config shallow-override the file (whole-key replace, not deep merge)', async () => {
    await write(
      'wvb.config.mjs',
      `export default {
         remote: { endpoint: 'https://some.where' },
         builtin: { clean: false, outDir: 'from-file' },
       };`
    );

    const resolved = await resolveConfig({
      root,
      remote: { endpoint: 'https://some.where' },
      builtin: { clean: true },
    });

    expect(resolved.remote?.endpoint).toBe('https://some.where');
    // Shallow merge: inline `builtin` replaces the file's `builtin` wholesale,
    // so `outDir` from the file is gone.
    expect(resolved.builtin?.clean).toBe(true);
    expect(resolved.builtin?.outDir).toBeUndefined();
  });

  it('skips file loading entirely when configFile is false', async () => {
    await write('wvb.config.mjs', `export default { remote: { endpoint: 'https://from-file' } };`);

    const resolved = await resolveConfig({
      root,
      configFile: false,
      remote: { endpoint: 'https://inline-only' },
    });

    expect(resolved.remote?.endpoint).toBe('https://inline-only');
    expect(resolved.configFile).toBeUndefined();
    expect(resolved.configFileDependencies).toEqual([]);
  });

  it('loads an explicit configFile path relative to root', async () => {
    await write('custom.config.mjs', `export default { remote: { endpoint: 'https://custom' } };`);

    const resolved = await resolveConfig({ root, configFile: './custom.config.mjs' });

    expect(resolved.remote?.endpoint).toBe('https://custom');
    expect(resolved.configFile).toBe(path.join(root, 'custom.config.mjs'));
  });

  it('defaults root to process.cwd() when omitted', async () => {
    // configFile:false avoids auto-discovering whatever config sits in the real cwd.
    const resolved = await resolveConfig({ configFile: false });

    expect(resolved.root).toBe(process.cwd());
  });
});

describe('loadConfigFile', () => {
  it('returns null when no config file is found', async () => {
    await expect(loadConfigFile(undefined, root)).resolves.toBeNull();
  });

  it('loads an ESM config (.mjs)', async () => {
    await write('wvb.config.mjs', `export default { remote: { endpoint: 'https://mjs' } };`);

    const loaded = await loadConfigFile(undefined, root);

    expect(loaded?.config.remote?.endpoint).toBe('https://mjs');
    expect(loaded?.configFile).toBe(path.join(root, 'wvb.config.mjs'));
  });

  it('loads a CommonJS config (.cjs) via the require hook', async () => {
    await write('wvb.config.cjs', `module.exports = { remote: { endpoint: 'https://cjs' } };`);

    const loaded = await loadConfigFile(undefined, root);

    expect(loaded?.config.remote?.endpoint).toBe('https://cjs');
  });

  it('transpiles a TypeScript config (.ts)', async () => {
    await write(
      'wvb.config.ts',
      `const endpoint: string = 'https://ts';\nexport default { remote: { endpoint } } satisfies { remote: { endpoint: string } };`
    );

    const loaded = await loadConfigFile(undefined, root);

    expect(loaded?.config.remote?.endpoint).toBe('https://ts');
  });

  it('parses a JSON config (.json)', async () => {
    await write('wvb.config.json', `{ "remote": { "endpoint": "https://json" } }`);

    const loaded = await loadConfigFile(undefined, root);

    expect(loaded?.config.remote?.endpoint).toBe('https://json');
  });

  it('honors DEFAULT_CONFIG_FILES precedence (.mjs before .json)', async () => {
    await write('wvb.config.mjs', `export default { remote: { endpoint: 'https://mjs' } };`);
    await write('wvb.config.json', `{ "remote": { "endpoint": "https://json" } }`);

    const loaded = await loadConfigFile(undefined, root);

    expect(loaded?.config.remote?.endpoint).toBe('https://mjs');
  });
});

describe('config default export forms', () => {
  it('invokes a sync factory function default export', async () => {
    await write(
      'wvb.config.mjs',
      `export default () => ({ remote: { endpoint: 'https://factory' } });`
    );

    const resolved = await resolveConfig({ root });

    expect(resolved.remote?.endpoint).toBe('https://factory');
  });

  it('awaits an async factory function default export', async () => {
    await write(
      'wvb.config.mjs',
      `export default async () => ({ remote: { endpoint: 'https://async-factory' } });`
    );

    const resolved = await resolveConfig({ root });

    expect(resolved.remote?.endpoint).toBe('https://async-factory');
  });

  it('awaits a Promise default export', async () => {
    await write(
      'wvb.config.mjs',
      `export default Promise.resolve({ remote: { endpoint: 'https://promise' } });`
    );

    const resolved = await resolveConfig({ root });

    expect(resolved.remote?.endpoint).toBe('https://promise');
  });

  it('invokes a factory exported from a CommonJS config (.cjs)', async () => {
    await write(
      'wvb.config.cjs',
      `module.exports = () => ({ remote: { endpoint: 'https://cjs-factory' } });`
    );

    const resolved = await resolveConfig({ root });

    expect(resolved.remote?.endpoint).toBe('https://cjs-factory');
  });

  it('normalizes the loaded config to a plain object, not the factory function', async () => {
    await write('wvb.config.mjs', `export default () => ({ builtin: { clean: false } });`);

    const loaded = await loadConfigFile(undefined, root);

    expect(typeof loaded?.config).toBe('object');
    expect(loaded?.config.builtin?.clean).toBe(false);
  });
});
