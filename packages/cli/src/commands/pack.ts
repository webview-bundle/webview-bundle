import type { HeadersConfig } from '@wvb/config';
import { Command, Option } from 'clipanion';
import { isNotNil } from 'es-toolkit';
import { isBoolean } from 'typanion';
import { pack } from '../api/pack.js';
import { resolveConfig, resolveOutDir, resolveOutFile } from '../config.js';
import { BaseCommand } from './base.js';

export class PackCommand extends BaseCommand {
  readonly name = 'pack';
  static paths = [['pack']];
  static usage = Command.Usage({
    description: 'Pack webview bundle archive.',
    examples: [
      ['A basic usage', '$0 pack ./dist'],
      ['Specify outfile path', '$0 pack ./dist --outfile ./dist.wvb'],
      ['Ignore files with patterns', `$0 pack ./dist --ignore='*.txt' --ignore='node_modules/**'`],
      ['Set headers for files', `$0 pack ./dist --header='*.html' 'cache-control' 'max-age=3600'`],
    ],
  });

  readonly srcDir = Option.String({ name: 'SRC_DIR', required: false });
  readonly outFile = Option.String('--outfile,--out-file,-O', {
    description: `Outfile name to create Webview Bundle archive.
If not provided, default to name field in "package.json" with normalized.
If extension not set, will automatically add extension (\`.wvb\`)`,
  });
  readonly outDir = Option.String('--outdir,--out-dir', {
    description: "Output directory for Webview Bundle archive. If not provided, default to '.wvb'",
  });
  readonly ignores = Option.Array('--ignore', {
    description: 'Ignore patterns.',
  });
  readonly headers = Option.Array('--header,-H', {
    description: `Headers to set for each file.
For example, \`--header '*.html' 'cache-control' 'max-age=3600'\` will set \`cache-control: max-age=3600\` for all files with extension \`.html\`.`,
    arity: 3,
  });
  readonly write = Option.String('--write', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: `Writing files on disk.
Set this to \`false\` (or pass "--no-write") just for simulating operation.
[Default: true]`,
  });
  readonly overwrite = Option.String('--overwrite', {
    validator: isBoolean(),
    tolerateBoolean: true,
    description: 'Overwrite outfile if file is already exists. [Default: true]',
  });
  readonly configFile = Option.String('--config,-C', {
    description: 'Path to the config file.',
  });
  readonly cwd = Option.String('--cwd', {
    description: 'Set the working directory for resolving paths. [Default: process.cwd()]',
  });

  async run() {
    const config = await resolveConfig({
      root: this.cwd,
      configFile: this.configFile,
    });
    const srcDir = this.srcDir ?? config.pack?.srcDir;
    if (srcDir == null) {
      this.logger.error(
        'Source directory is not specified. Set "pack.srcDir" in the config file or pass [SRC_DIR] as a CLI argument.'
      );
      return 1;
    }
    const outFile = this.outFile ?? resolveOutFile(config);
    if (outFile == null) {
      this.logger.error(
        'Out file is not specified. Set "pack.outFile" in the config file or pass "--outfile,--out-file,-O" as a CLI argument.'
      );
      return 1;
    }
    const outDir = this.outDir ?? resolveOutDir(config);
    const overwrite = this.overwrite ?? config.pack?.overwrite ?? true;
    await pack({
      srcDir,
      outFile,
      outDir,
      ignores: [this.ignores, config.pack?.ignore].filter(isNotNil),
      headers: [
        config.pack?.headers,
        this.headers != null ? this.intoHeaderConfig(this.headers) : undefined,
      ].filter(isNotNil),
      write: this.write,
      overwrite,
      cwd: config.root,
      logLevel: this.logLevel,
      logger: this.logger,
    });
  }

  private intoHeaderConfig(headers: [string, string, string][]): HeadersConfig {
    const config: Record<string, [string, string][]> = {};
    for (const [pattern, key, value] of headers) {
      if (config[pattern] == null) {
        config[pattern] = [[key, value]];
      } else {
        config[pattern]!.push([key, value]);
      }
    }
    return config;
  }
}
