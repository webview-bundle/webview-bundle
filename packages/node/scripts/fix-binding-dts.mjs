// Post-build: rewrite the `#binding` subpath import in the emitted declaration files to a concrete
// relative path.
//
// The runtime (`dist/index.js` / `dist/index.cjs`) must keep `#binding` so Node picks
// `binding.js` vs `binding.cjs` per format at load time. But TypeScript's classic (`node10`)
// resolution doesn't understand the package `imports` field, so `#binding` in the shipped `.d.ts`
// fails to resolve there (attw flags an InternalResolutionError). A concrete relative path resolves
// under node10, node16/nodenext, and bundler alike — verified across all three — so only the
// declaration files are rewritten; the JS is left untouched.
import { readFile, writeFile } from 'node:fs/promises';

const edits = [
  { file: new URL('../dist/index.d.ts', import.meta.url), to: '../binding.js' },
  { file: new URL('../dist/index.d.cts', import.meta.url), to: '../binding.cjs' },
];

for (const { file, to } of edits) {
  const before = await readFile(file, 'utf8');
  const after = before.replaceAll('"#binding"', `"${to}"`);
  if (after === before) {
    throw new Error(`fix-binding-dts: no \`#binding\` import found in ${file.pathname}`);
  }
  await writeFile(file, after);
}
