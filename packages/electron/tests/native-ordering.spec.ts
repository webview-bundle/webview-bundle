import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

// `native.ts` sets `NAPI_RS_NATIVE_LIBRARY_PATH` and MUST run before `@wvb/node`
// loads its binding, otherwise the override is ignored and `@wvb/node` silently
// falls back to its own resolution — which still works on the build host, so the
// e2e cannot catch a regression here. Guard the emitted import order directly.
const distDir = path.join(import.meta.dirname, '..', 'dist');

function orderingOk(source: string, nativeMarker: RegExp, nodeMarker: RegExp): boolean {
  const native = source.search(nativeMarker);
  const node = source.search(nodeMarker);
  return native !== -1 && node !== -1 && native < node;
}

describe('native side effect load order', () => {
  it.runIf(existsSync(path.join(distDir, 'index.mjs')))(
    'imports ./native before @wvb/node in the ESM entry',
    () => {
      const source = readFileSync(path.join(distDir, 'index.mjs'), 'utf8');
      expect(
        orderingOk(source, /import\s*["']\.\/native\.mjs["']/, /from\s*["']@wvb\/node["']/)
      ).toBe(true);
    }
  );

  it.runIf(existsSync(path.join(distDir, 'index.cjs')))(
    'requires ./native before @wvb/node in the CJS entry',
    () => {
      const source = readFileSync(path.join(distDir, 'index.cjs'), 'utf8');
      expect(
        orderingOk(source, /require\(["']\.\/native\.cjs["']\)/, /require\(["']@wvb\/node["']\)/)
      ).toBe(true);
    }
  );
});
