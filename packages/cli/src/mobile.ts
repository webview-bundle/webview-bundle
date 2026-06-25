import fs from 'node:fs/promises';
import path from 'node:path';
import { pathExists } from './fs.js';

/**
 * Default builtin staging path under an Android app module's main source set. Files placed here are
 * merged into the APK/AAB `assets/` by AGP; at runtime they must be extracted to a real filesystem
 * directory (e.g. `filesDir`) because assets are not filesystem paths.
 */
export const ANDROID_BUILTIN_OUT = path.join('src', 'main', 'assets', 'bundles', 'builtin');

export type AndroidNoCompressStatus = 'ok' | 'missing' | 'skipped';

/**
 * Best-effort check that an Android module keeps `.wvb` assets uncompressed (`noCompress`), so the
 * already-compressed bundles aren't wastefully re-compressed in the APK. Scans the module's
 * `build.gradle` / `build.gradle.kts`. Returns:
 * - `'ok'`: a gradle file mentions `noCompress` together with a quoted `wvb`.
 * - `'missing'`: a gradle file exists but doesn't.
 * - `'skipped'`: no gradle file was found to inspect.
 */
export async function checkAndroidNoCompress(moduleDir: string): Promise<AndroidNoCompressStatus> {
  const candidates = [
    path.join(moduleDir, 'build.gradle.kts'),
    path.join(moduleDir, 'build.gradle'),
  ];
  let scanned = false;
  for (const file of candidates) {
    if (!(await pathExists(file))) {
      continue;
    }
    scanned = true;
    let text: string;
    try {
      text = await fs.readFile(file, 'utf8');
    } catch {
      continue;
    }
    // Look for `noCompress` and a quoted "wvb" — quoted form avoids matching the `dev.wvb` namespace.
    if (/noCompress/.test(text) && /["']wvb["']/.test(text)) {
      return 'ok';
    }
  }
  return scanned ? 'missing' : 'skipped';
}
