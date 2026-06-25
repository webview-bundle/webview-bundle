import fs from 'node:fs/promises';
import path from 'node:path';
import { pathExists } from './fs.js';

/**
 * Default builtin staging path under an Android app module's main source set. Files placed here are
 * merged into the APK/AAB `assets/` by AGP; at runtime they must be extracted to a real filesystem
 * directory (e.g. `filesDir`) because assets are not filesystem paths.
 */
export const ANDROID_BUILTIN_OUT = path.join('src', 'main', 'assets', 'bundles', 'builtin');

/**
 * Default builtin staging path under an iOS project's folder-reference root. Matches the runtime
 * contract (`Bundle.main.resourceURL` + `assets/bundles/builtin`) and the committed example layout.
 */
export const IOS_BUILTIN_OUT = path.join('assets', 'bundles', 'builtin');

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

// --- Android application-module auto-detection -------------------------------------------------

export interface AndroidProject {
  /** Gradle root directory (the one with `settings.gradle(.kts)`), or the module dir if single-module. */
  root: string;
  /** The application module directory (the one with `src/main/assets`). */
  moduleDir: string;
  /** The Gradle module name (e.g. `app`, `testapp`). */
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

/**
 * Locate the Android *application* module (the one with `com.android.application` and `src/main/assets`)
 * without an explicit path — mirrors {@link import('./tauri.js').resolveTauriProject}. When `explicitDir`
 * is given it is used as the module directly (still validated). Otherwise it searches `cwd`, conventional
 * module subdirs (`app`, `android/app`, `src-tauri/gen/android/app`), Gradle roots (`.`, `android`,
 * `src-tauri/gen/android`), and a few parent levels — so it works from a frontend dir or the project root.
 */
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

  let dir = path.resolve(cwd);
  for (let i = 0; i < 4; i++) {
    // Fast path: a conventionally-named app module directly under common toolchain locations.
    for (const rel of [
      'app',
      path.join('android', 'app'),
      path.join('src-tauri', 'gen', 'android', 'app'),
    ]) {
      const moduleDir = path.join(dir, rel);
      if (await isAndroidAppModule(moduleDir)) {
        return { root: path.dirname(moduleDir), moduleDir, moduleName: path.basename(moduleDir) };
      }
    }
    // General path: find a Gradle root and classify its modules (handles non-`app` module names).
    for (const rel of ['', 'android', path.join('src-tauri', 'gen', 'android')]) {
      const found = await findAppModuleInGradleRoot(path.join(dir, rel));
      if (found != null) {
        return found;
      }
    }
    const parent = path.dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return null;
}

// --- iOS project auto-detection ----------------------------------------------------------------

export type IosProjectKind = 'tuist' | 'workspace' | 'xcodeproj';

export interface IosProject {
  /** The iOS project directory (the folder-reference parent; bundles stage under it). */
  dir: string;
  kind: IosProjectKind;
  /** Folder-reference root dir name (the Tuist `.folderReference` path, default `assets`). */
  folderReferenceRoot: string;
}

async function readdirSafe(dir: string): Promise<string[]> {
  try {
    return await fs.readdir(dir);
  } catch {
    return [];
  }
}

async function detectIosProject(dir: string): Promise<IosProject | null> {
  // Tuist's `Project.swift` is the COMMITTED source of truth; the `.xcodeproj`/`.xcworkspace` it
  // generates are typically gitignored, so detect it first.
  if (await pathExists(path.join(dir, 'Project.swift'))) {
    let folderReferenceRoot = 'assets';
    try {
      const src = await fs.readFile(path.join(dir, 'Project.swift'), 'utf8');
      const captured = src.match(/\.folderReference\s*\(\s*path:\s*['"]\.?\/?([^'"]+)['"]/)?.[1];
      if (captured != null) {
        folderReferenceRoot = captured.split(/[/\\]/)[0] || 'assets';
      }
    } catch {
      // fall back to the default root
    }
    return { dir, kind: 'tuist', folderReferenceRoot };
  }

  const entries = await readdirSafe(dir);
  if (entries.some(e => e.endsWith('.xcworkspace'))) {
    return { dir, kind: 'workspace', folderReferenceRoot: 'assets' };
  }
  if (entries.some(e => e.endsWith('.xcodeproj') && e !== 'Pods.xcodeproj')) {
    return { dir, kind: 'xcodeproj', folderReferenceRoot: 'assets' };
  }
  return null;
}

/**
 * Locate the iOS project directory without an explicit path — mirrors `resolveTauriProject`. Detects
 * Tuist (`Project.swift`), then `*.xcworkspace`, then `*.xcodeproj`, searching `cwd`, `ios`,
 * `apple/ios`, `src-tauri/gen/apple`, and a few parent levels. Returns the project dir; the bundles
 * stage at `<dir>/<folderReferenceRoot>/bundles/builtin`. Does NOT auto-wire the Xcode folder
 * reference — that stays a documented manual step.
 */
export async function resolveIosProject(
  cwd: string,
  explicitDir?: string
): Promise<IosProject | null> {
  if (explicitDir != null) {
    return detectIosProject(path.resolve(cwd, explicitDir));
  }

  let dir = path.resolve(cwd);
  for (let i = 0; i < 4; i++) {
    for (const rel of [
      '',
      'ios',
      path.join('apple', 'ios'),
      path.join('src-tauri', 'gen', 'apple'),
    ]) {
      const found = await detectIosProject(path.join(dir, rel));
      if (found != null) {
        return found;
      }
    }
    const parent = path.dirname(dir);
    if (parent === dir) {
      break;
    }
    dir = parent;
  }
  return null;
}

/** The default builtin staging directory for a detected iOS project. */
export function iosStagingDir(project: IosProject): string {
  return path.join(project.dir, project.folderReferenceRoot, 'bundles', 'builtin');
}
