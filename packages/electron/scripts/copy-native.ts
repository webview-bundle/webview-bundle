import { existsSync } from 'node:fs';
import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { NATIVE_TARGETS, type NativeTarget } from '../src/native-targets.ts';

const require = createRequire(import.meta.url);
const nodePkgDir = path.dirname(require.resolve('@wvb/node/package.json'));
const electronPkgDir = path.join(import.meta.dirname, '..');
const destDir = path.join(electronPkgDir, 'native');

// `.node` binaries can live in a few places depending on the context:
//   - `<node>/artifacts/*.node`  — every arch, right after `xtask artifacts spread` in a release
//   - `<node>/npm/<suffix>/*.node`— every published arch, after `yarn artifacts`
//   - `<node>/*.node`            — the host arch only, after a local `napi build`
// We copy whichever exists so local dev works with just the host arch while a
// release bundles all of them.
function candidateSources(target: NativeTarget): string[] {
  const sources = [
    path.join(nodePkgDir, 'artifacts', target.file),
    path.join(nodePkgDir, 'npm', target.npmSuffix, target.file),
    path.join(nodePkgDir, target.file),
  ];
  try {
    const depDir = path.dirname(require.resolve(`@wvb/node-${target.npmSuffix}/package.json`));
    sources.push(path.join(depDir, target.file));
  } catch {
    // The optional dependency may not be installed; ignore.
  }
  return sources;
}

const strict = process.env.WVB_BUNDLE_NATIVE_STRICT === '1';

await fs.rm(destDir, { recursive: true, force: true });
await fs.mkdir(destDir, { recursive: true });

const copied: string[] = [];
const missing: NativeTarget[] = [];

for (const target of NATIVE_TARGETS) {
  const src = candidateSources(target).find(candidate => existsSync(candidate));
  if (src == null) {
    missing.push(target);
    console.warn(`[copy-native] missing ${target.file} (${target.triple})`);
    continue;
  }
  await fs.copyFile(src, path.join(destDir, target.file));
  copied.push(target.file);
  console.log(`[copy-native] ${target.file} <- ${path.relative(electronPkgDir, src)}`);
}

console.log(`[copy-native] copied ${copied.length}/${NATIVE_TARGETS.length} binaries`);

if (strict && missing.length > 0) {
  console.error(
    `[copy-native] strict mode: ${missing.length} required binaries missing: ${missing
      .map(target => target.file)
      .join(', ')}`
  );
  process.exit(1);
}
