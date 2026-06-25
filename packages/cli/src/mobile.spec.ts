import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  ANDROID_BUILTIN_OUT,
  checkAndroidNoCompress,
  IOS_BUILTIN_OUT,
  iosStagingDir,
  resolveAndroidProject,
  resolveIosProject,
} from './mobile.js';

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

const MANIFEST = '<manifest/>\n';
const APP_KTS =
  'plugins {\n  alias(libs.plugins.android.application)\n}\nandroid {\n  defaultConfig { applicationId = "dev.wvb.app" }\n}\n';
const LIB_KTS =
  'plugins {\n  alias(libs.plugins.android.library)\n}\nandroid {\n  namespace = "dev.wvb.lib"\n}\n';

describe('resolveAndroidProject', () => {
  it('finds the application module by name from a multi-module Gradle root (catalog alias)', async () => {
    await write('settings.gradle.kts', 'include(":testapp", ":lib-android")\n');
    await write('testapp/build.gradle.kts', APP_KTS);
    await write('testapp/src/main/AndroidManifest.xml', MANIFEST);
    await write('lib-android/build.gradle.kts', LIB_KTS);
    await write('lib-android/src/main/AndroidManifest.xml', MANIFEST);

    const project = await resolveAndroidProject(root);
    expect(project?.moduleName).toBe('testapp');
    expect(project?.moduleDir).toBe(path.join(root, 'testapp'));
  });

  it('detects a conventional `app` module via the literal plugin id (no settings file)', async () => {
    await write('app/build.gradle.kts', 'plugins {\n  id("com.android.application")\n}\n');
    await write('app/src/main/AndroidManifest.xml', MANIFEST);
    const project = await resolveAndroidProject(root);
    expect(project?.moduleDir).toBe(path.join(root, 'app'));
  });

  it('detects the Groovy `apply plugin` form', async () => {
    await write('app/build.gradle', "apply plugin: 'com.android.application'\n");
    await write('app/src/main/AndroidManifest.xml', MANIFEST);
    expect((await resolveAndroidProject(root))?.moduleName).toBe('app');
  });

  it('rejects a library-only project (returns null)', async () => {
    await write('settings.gradle.kts', 'include(":lib-android")\n');
    await write('lib-android/build.gradle.kts', LIB_KTS);
    await write('lib-android/src/main/AndroidManifest.xml', MANIFEST);
    expect(await resolveAndroidProject(root)).toBeNull();
  });

  it('walks up from a nested cwd', async () => {
    await write('settings.gradle.kts', 'include(":app")\n');
    await write('app/build.gradle.kts', APP_KTS);
    await write('app/src/main/AndroidManifest.xml', MANIFEST);
    await fs.mkdir(path.join(root, 'frontend', 'src'), { recursive: true });
    expect((await resolveAndroidProject(path.join(root, 'frontend')))?.moduleName).toBe('app');
  });

  it('honors an explicit module dir and validates it', async () => {
    await write('mymod/build.gradle.kts', APP_KTS);
    await write('mymod/src/main/AndroidManifest.xml', MANIFEST);
    await write('lib/build.gradle.kts', LIB_KTS);
    await write('lib/src/main/AndroidManifest.xml', MANIFEST);
    expect((await resolveAndroidProject(root, 'mymod'))?.moduleDir).toBe(path.join(root, 'mymod'));
    // A wrong explicit dir (a library) still fails clearly rather than staging into a non-app dir.
    expect(await resolveAndroidProject(root, 'lib')).toBeNull();
  });
});

describe('resolveIosProject', () => {
  it('detects a Tuist project via Project.swift and honors the folderReference root', async () => {
    await write(
      'Project.swift',
      'let project = Project(resources: [.folderReference(path: "./assets")])\n'
    );
    const project = await resolveIosProject(root);
    expect(project?.kind).toBe('tuist');
    expect(project?.folderReferenceRoot).toBe('assets');
    expect(project?.dir).toBe(root);
  });

  it('detects an .xcodeproj (and skips Pods.xcodeproj)', async () => {
    await write('Pods.xcodeproj/project.pbxproj', '// pods\n');
    await write('MyApp.xcodeproj/project.pbxproj', '// app\n');
    expect((await resolveIosProject(root))?.kind).toBe('xcodeproj');
  });

  it('walks up to an `ios` subdir and honors an explicit dir', async () => {
    await write('ios/Project.swift', 'let project = Project()\n');
    expect((await resolveIosProject(root))?.dir).toBe(path.join(root, 'ios'));
    expect((await resolveIosProject(root, 'ios'))?.kind).toBe('tuist');
  });

  it('returns null when no iOS project marker is found', async () => {
    await write('placeholder.txt', 'x');
    expect(await resolveIosProject(root)).toBeNull();
  });

  it('iosStagingDir uses the folder-reference root + IOS_BUILTIN_OUT layout', () => {
    expect(IOS_BUILTIN_OUT).toBe(path.join('assets', 'bundles', 'builtin'));
    expect(iosStagingDir({ dir: '/x', kind: 'tuist', folderReferenceRoot: 'Resources' })).toBe(
      path.join('/x', 'Resources', 'bundles', 'builtin')
    );
  });
});
