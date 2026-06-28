import fs from 'node:fs/promises';
import path from 'node:path';
import { pathExists } from './fs.js';

export type AndroidNoCompressStatus = 'ok' | 'missing' | 'skipped';

async function readModuleGradle(moduleDir: string): Promise<string | null> {
  for (const file of ['build.gradle.kts', 'build.gradle']) {
    const filepath = path.join(moduleDir, file);
    if (await pathExists(filepath)) {
      try {
        return await fs.readFile(filepath, 'utf8');
      } catch {
        return null;
      }
    }
  }
  return null;
}

/**
 * Best-effort check that an Android module keeps `.wvb` assets uncompressed (`noCompress`), so the
 * already-compressed bundles aren't wastefully re-compressed in the APK. Scans the module's
 * `build.gradle` / `build.gradle.kts`. Returns:
 * - `'ok'`: a gradle file mentions `noCompress` together with a quoted `wvb`.
 * - `'missing'`: a gradle file exists but doesn't.
 * - `'skipped'`: no gradle file was found to inspect.
 */
export async function checkAndroidNoCompress(moduleDir: string): Promise<AndroidNoCompressStatus> {
  const text = await readModuleGradle(moduleDir);
  if (text == null) {
    return 'skipped';
  }
  // Require a quoted "wvb" close to `noCompress` (same statement) rather than anywhere in the file,
  // so an unrelated `noCompress` for another extension + a stray "wvb" elsewhere isn't a match.
  // The quoted form also avoids matching the `dev.wvb` namespace.
  return /noCompress\b[\s\S]{0,60}["']wvb["']/.test(text) ? 'ok' : 'missing';
}

export interface AndroidProject {
  root: string;
  moduleDir: string;
  moduleName: string;
}

// The application plugin appears as a version-catalog alias, a literal id, or a legacy apply — match all.
const ANDROID_APP_PLUGIN =
  /alias\s*\(\s*libs\.plugins\.android\.application\s*\)|id\s*\(?\s*['"]com\.android\.application['"]|apply\s+plugin:\s*['"]com\.android\.application['"]/;
const ANDROID_LIB_PLUGIN =
  /alias\s*\(\s*libs\.plugins\.android\.library\s*\)|id\s*\(?\s*['"]com\.android\.library['"]|apply\s+plugin:\s*['"]com\.android\.library['"]/;
// `applicationId` is application-exclusive (libraries have `namespace`, not `applicationId`).
const ANDROID_APPLICATION_ID = /\bapplicationId\s*[=\s]\s*['"]/;

/** Is `moduleDir` an Android *application* module (has a source manifest + app markers, not a library)? */
async function isAndroidAppModule(moduleDir: string): Promise<boolean> {
  if (!(await pathExists(path.join(moduleDir, 'src', 'main', 'AndroidManifest.xml')))) {
    return false;
  }
  const gradle = await readModuleGradle(moduleDir);
  if (gradle == null) {
    return false;
  }
  const isApp = ANDROID_APP_PLUGIN.test(gradle) || ANDROID_APPLICATION_ID.test(gradle);
  const isLib = ANDROID_LIB_PLUGIN.test(gradle) && !ANDROID_APPLICATION_ID.test(gradle);
  return isApp && !isLib;
}

/** Extract Gradle module paths from a `settings.gradle(.kts)` `include(...)` list (e.g. `:app`, `:a:b`). */
function parseGradleIncludes(text: string): string[] {
  const modules: string[] = [];
  for (const line of text.matchAll(/^\s*include\b(.*)$/gm)) {
    const args = line[1];
    if (args == null) {
      continue;
    }
    for (const token of args.matchAll(/['"]([^'"]+)['"]/g)) {
      const value = token[1];
      if (value != null) {
        modules.push(value);
      }
    }
  }
  return modules;
}

async function findAppModuleInGradleRoot(root: string): Promise<AndroidProject | null> {
  const settings = (await pathExists(path.join(root, 'settings.gradle.kts')))
    ? path.join(root, 'settings.gradle.kts')
    : (await pathExists(path.join(root, 'settings.gradle')))
      ? path.join(root, 'settings.gradle')
      : null;

  // No settings file → single-module project: the root itself may be the application module.
  if (settings == null) {
    return (await isAndroidAppModule(root))
      ? { root, moduleDir: root, moduleName: path.basename(root) }
      : null;
  }

  let text: string;
  try {
    text = await fs.readFile(settings, 'utf8');
  } catch {
    return null;
  }

  const appModules: AndroidProject[] = [];
  for (const mod of parseGradleIncludes(text)) {
    const rel = mod.replace(/^:/, '').split(':').join(path.sep);
    const moduleDir = path.join(root, rel);
    if (await isAndroidAppModule(moduleDir)) {
      appModules.push({ root, moduleDir, moduleName: rel.split(path.sep).pop() ?? rel });
    }
  }

  if (appModules.length === 1) {
    return appModules[0] ?? null;
  }
  if (appModules.length > 1) {
    // Multi-app project: only auto-pick a module literally named `app`; otherwise it's ambiguous.
    return appModules.find(m => m.moduleName === 'app') ?? null;
  }
  return null;
}

export async function resolveAndroidProject(
  cwd: string,
  explicitDir?: string
): Promise<AndroidProject | null> {
  if (explicitDir != null) {
    const moduleDir = path.resolve(cwd, explicitDir);
    return (await isAndroidAppModule(moduleDir))
      ? { root: path.dirname(moduleDir), moduleDir, moduleName: path.basename(moduleDir) }
      : null;
  }

  const dir = path.resolve(cwd);
  for (const rel of ['app', path.join('android', 'app')]) {
    const moduleDir = path.join(dir, rel);
    if (await isAndroidAppModule(moduleDir)) {
      return { root: path.dirname(moduleDir), moduleDir, moduleName: path.basename(moduleDir) };
    }
  }
  // General path: find a Gradle root and classify its modules (handles non-`app` module names).
  for (const rel of ['', 'android']) {
    const found = await findAppModuleInGradleRoot(path.join(dir, rel));
    if (found != null) {
      return found;
    }
  }

  return null;
}

export function defaultAndroidBundlesDir(project: AndroidProject): string {
  return path.join(project.moduleDir, 'src', 'main', 'assets', 'bundles');
}

export type IosProjectKind = 'tuist' | 'workspace' | 'xcodeproj';

export interface IosProject {
  /** The iOS project directory (where `Project.swift` / the Xcode project lives; bundles stage under it). */
  dir: string;
  kind: IosProjectKind;
}

async function safeReaddir(dir: string): Promise<string[]> {
  try {
    return await fs.readdir(dir);
  } catch {
    return [];
  }
}

async function detectIosProject(dir: string): Promise<IosProject | null> {
  if (await pathExists(path.join(dir, 'Project.swift'))) {
    return { dir, kind: 'tuist' };
  }

  const entries = await safeReaddir(dir);
  if (entries.some(e => e.endsWith('.xcworkspace'))) {
    return { dir, kind: 'workspace' };
  }
  if (entries.some(e => e.endsWith('.xcodeproj') && e !== 'Pods.xcodeproj')) {
    return { dir, kind: 'xcodeproj' };
  }
  return null;
}

export async function resolveIosProject(
  cwd: string,
  explicitDir?: string
): Promise<IosProject | null> {
  if (explicitDir != null) {
    return detectIosProject(path.resolve(cwd, explicitDir));
  }

  const dir = path.resolve(cwd);
  for (const rel of ['', 'ios', path.join('apple', 'ios')]) {
    const found = await detectIosProject(path.join(dir, rel));
    if (found != null) {
      return found;
    }
  }

  return null;
}

/** The default builtin staging directory for a detected iOS project (`<dir>/bundles`). */
export function defaultIosProjectBundlesDir(project: IosProject): string {
  return path.join(project.dir, 'bundles');
}

export type IosAddFolderReferenceStatus = 'added' | 'already' | 'no-resources' | 'not-found';

export async function addIosFolderReference(
  projectDir: string,
  folder: string
): Promise<IosAddFolderReferenceStatus> {
  const file = path.join(projectDir, 'Project.swift');
  let src: string;
  try {
    src = await fs.readFile(file, 'utf8');
  } catch {
    return 'not-found';
  }

  // Idempotency: compare the whole (normalized) referenced path, not just its basename — a reference to
  // a different directory that merely ends in `<folder>` (e.g. `../shared/bundles`) is not a match.
  for (const match of src.matchAll(/\.folderReference\s*\(\s*path:\s*['"]([^'"]+)['"]/g)) {
    const ref = match[1]?.replace(/^\.\//, '').replace(/[/\\]+$/, '');
    if (ref === folder) {
      return 'already';
    }
  }

  const resources = src.match(/resources:\s*\[/);
  if (resources?.index == null) {
    return 'no-resources';
  }
  const insertAt = resources.index + resources[0].length;
  const lineStart = src.lastIndexOf('\n', resources.index) + 1;
  const indent = src.slice(lineStart, resources.index).match(/^\s*/)?.[0] ?? '';
  const entry = `\n${indent}    .folderReference(path: "./${folder}"),`;
  await fs.writeFile(file, src.slice(0, insertAt) + entry + src.slice(insertAt));
  return 'added';
}
