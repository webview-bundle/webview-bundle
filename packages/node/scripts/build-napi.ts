import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { NapiCli } from '@napi-rs/cli';
import yargs from 'yargs';

const args = await yargs(process.argv)
  .option('target', {
    alias: 't',
    type: 'string',
  })
  .option('crossCompile', {
    alias: 'x',
    type: 'boolean',
  })
  .option('useCross', {
    type: 'boolean',
  })
  .option('useNapiCross', {
    type: 'boolean',
  })
  .parse();

const cli = new NapiCli();

const { task } = await cli.build({
  cwd: path.join(import.meta.dirname, '..'),
  platform: true,
  release: true,
  jsBinding: 'binding.cjs',
  dts: 'binding.d.cts',
  constEnum: false,
  dtsCache: false,
  esm: false,
  target: args.target,
  crossCompile: args.crossCompile,
  useCross: args.useCross,
  useNapiCross: args.useNapiCross,
});

const outputs = await task;

for (const { kind, path: file } of outputs) {
  if (kind === 'dts') {
    await writeFile(file, unwrapOutcome(await readFile(file, 'utf8')));
  }
}

// `Outcome<T>` is a Rust-side wrapper (see src/error.rs) that must not reach the published types,
// and napi offers no hook to rewrite a generated type — so strip it from the .d.cts here.
function unwrapOutcome(dts: string): string {
  const WRAPPER = 'Outcome<';
  let out = '';
  let rest = dts;
  for (;;) {
    const at = rest.indexOf(WRAPPER);
    if (at === -1) {
      return out + rest;
    }
    // Skip identifiers that merely end in `Outcome`, e.g. a future `BuildOutcome<T>`.
    if (/[\w$]/.test(rest[at - 1] ?? '')) {
      out += rest.slice(0, at + WRAPPER.length);
      rest = rest.slice(at + WRAPPER.length);
      continue;
    }
    const open = at + WRAPPER.length - 1;
    let depth = 0;
    let close = -1;
    for (let i = open; i < rest.length; i++) {
      if (rest[i] === '<') {
        depth++;
      } else if (rest[i] === '>' && --depth === 0) {
        close = i;
        break;
      }
    }
    if (close === -1) {
      throw new Error(`unbalanced ${WRAPPER}> in the generated type definitions`);
    }
    out += rest.slice(0, at);
    // Re-scan the inner type so nested wrappers collapse and the payload renders as a return type.
    rest = asReturnType(rest.slice(open + 1, close)) + rest.slice(close + 1);
  }
}

// napi renders a type differently as a nested argument than in return position (`Option<T>` ->
// `T | undefined | null` vs `T | null`, `()` -> `undefined` vs `void`); the wrapper nested it, so
// undo that. An `Outcome` only ever stands where a call yields its value, so this is always right.
function asReturnType(ty: string): string {
  if (ty === 'undefined') {
    return 'void';
  }
  const OPTIONAL = ' | undefined | null';
  const head = ty.slice(0, -OPTIONAL.length);
  return ty.endsWith(OPTIONAL) && isBalanced(head) ? `${head} | null` : ty;
}

function isBalanced(ty: string): boolean {
  let depth = 0;
  for (const c of ty) {
    if (c === '<' || c === '(' || c === '[' || c === '{') {
      depth++;
    } else if (c === '>' || c === ')' || c === ']' || c === '}') {
      depth--;
    }
  }
  return depth === 0;
}
