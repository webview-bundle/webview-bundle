import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { checkBundleResources, resolveTauriProject } from './tauri.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-tauri-'));
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

async function write(rel: string, content: string) {
  const file = path.join(root, rel);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, content);
  return file;
}

describe('resolveTauriProject', () => {
  it('finds src-tauri/tauri.conf.json from the project root', async () => {
    const configFile = await write('src-tauri/tauri.conf.json', '{}');
    const project = await resolveTauriProject(root);
    expect(project?.dir).toBe(path.join(root, 'src-tauri'));
    expect(project?.configFile).toBe(configFile);
  });

  it('finds tauri.conf.json when cwd is the project dir itself', async () => {
    const configFile = await write('tauri.conf.json', '{}');
    const project = await resolveTauriProject(root);
    expect(project?.dir).toBe(root);
    expect(project?.configFile).toBe(configFile);
  });

  it('walks up from a nested frontend dir (beforeBundleCommand cwd)', async () => {
    await write('src-tauri/tauri.conf.json', '{}');
    const frontend = path.join(root, 'apps', 'web');
    await fs.mkdir(frontend, { recursive: true });
    const project = await resolveTauriProject(frontend);
    expect(project?.dir).toBe(path.join(root, 'src-tauri'));
  });

  it('honors an explicit --tauri-dir', async () => {
    const configFile = await write('custom/tauri.conf.json5', '{}');
    const project = await resolveTauriProject(root, 'custom');
    expect(project?.dir).toBe(path.join(root, 'custom'));
    expect(project?.configFile).toBe(configFile);
  });

  it('returns null when no Tauri config exists', async () => {
    expect(await resolveTauriProject(root)).toBeNull();
  });
});

describe('checkBundleResources', () => {
  it('returns "ok" when a resources array glob references the bundles dir', async () => {
    const file = await write(
      'tauri.conf.json',
      JSON.stringify({ bundle: { resources: ['bundles/**/*.wvb', 'bundles/manifest.json'] } })
    );
    expect(await checkBundleResources(file)).toBe('ok');
  });

  it('returns "ok" for the map/object form', async () => {
    const file = await write(
      'tauri.conf.json',
      JSON.stringify({ bundle: { resources: { 'bundles/': 'bundles' } } })
    );
    expect(await checkBundleResources(file)).toBe('ok');
  });

  it('returns "missing" when resources is absent', async () => {
    const file = await write('tauri.conf.json', JSON.stringify({ bundle: { active: true } }));
    expect(await checkBundleResources(file)).toBe('missing');
  });

  it('returns "missing" when no entry references the bundles dir', async () => {
    const file = await write(
      'tauri.conf.json',
      JSON.stringify({ bundle: { resources: ['assets/**/*'] } })
    );
    expect(await checkBundleResources(file)).toBe('missing');
  });

  it('does not match a substring like "mybundles"', async () => {
    const file = await write(
      'tauri.conf.json',
      JSON.stringify({ bundle: { resources: ['mybundles/x.bin'] } })
    );
    expect(await checkBundleResources(file)).toBe('missing');
  });

  it('skips TOML configs', async () => {
    const file = await write('Tauri.toml', '[bundle]\nactive = true\n');
    expect(await checkBundleResources(file)).toBe('skipped');
  });

  it('skips unparsable configs', async () => {
    const file = await write('tauri.conf.json', '{ not valid json,, }');
    expect(await checkBundleResources(file)).toBe('skipped');
  });
});
