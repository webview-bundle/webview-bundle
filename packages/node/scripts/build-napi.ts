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

await cli.build({
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
