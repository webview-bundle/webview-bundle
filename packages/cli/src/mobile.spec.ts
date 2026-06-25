import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { ANDROID_BUILTIN_OUT, checkAndroidNoCompress } from './mobile.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-mobile-'));
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

async function write(rel: string, content: string) {
  const file = path.join(root, rel);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, content);
}

describe('ANDROID_BUILTIN_OUT', () => {
  it('is the merged-assets builtin path under a module', () => {
    expect(ANDROID_BUILTIN_OUT).toBe(path.join('src', 'main', 'assets', 'bundles', 'builtin'));
  });
});

describe('checkAndroidNoCompress', () => {
  it('returns "ok" when build.gradle.kts keeps wvb uncompressed', async () => {
    await write('build.gradle.kts', 'android {\n  androidResources { noCompress += "wvb" }\n}\n');
    expect(await checkAndroidNoCompress(root)).toBe('ok');
  });

  it('returns "ok" for the Groovy build.gradle form', async () => {
    await write('build.gradle', "android {\n  androidResources { noCompress 'wvb' }\n}\n");
    expect(await checkAndroidNoCompress(root)).toBe('ok');
  });

  it('returns "missing" when noCompress for wvb is absent', async () => {
    await write('build.gradle.kts', 'android {\n  namespace = "dev.wvb.app"\n}\n');
    expect(await checkAndroidNoCompress(root)).toBe('missing');
  });

  it('does not false-positive on the dev.wvb namespace alone', async () => {
    await write(
      'build.gradle.kts',
      'android {\n  namespace = "dev.wvb.app"\n  // no noCompress\n}\n'
    );
    expect(await checkAndroidNoCompress(root)).toBe('missing');
  });

  it('returns "skipped" when there is no gradle file', async () => {
    expect(await checkAndroidNoCompress(root)).toBe('skipped');
  });
});
