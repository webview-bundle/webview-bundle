import type { HeadersConfig } from '@wvb/config';
import { Command, Option } from 'clipanion';
import { isNotNil } from 'es-toolkit';
import { isBoolean } from 'typanion';
import { create } from '../api/create.js';
import { defaultOutDir, defaultOutFile, resolveConfig } from '../config.js';
import { BaseCommand } from './base.js';

export class CreateCommand extends BaseCommand {
  readonly name = 'create';
  static paths = [['create']];
  static usage = Command.Usage({
    description: 'Create webview bundle archive.',
    examples: [
      ['A basic usage', '$0 create ./dist'],
      ['Specify outfile path', '$0 create ./dist --outfile ./dist.wvb'],
      [
        'Ignore files with patterns',
        `$0 create ./dist --ignore='*.txt' --ignore='node_modules/**'`,
      ],
      [
        'Set headers for files',
        `$0 create ./dist --header='*.html' 'cache-control' 'max-age=3600'`,
      ],
    ],
  });

  readonly dir = Option.String({ name: 'DIR', required: false });
  readonly outFile = Option.String('--outfile,-O', {
    description: `Outfile name to create Webview Bundle archive.
If not provided, default to name field in "package.json" with normalized.
If extension not set, will automatically add extension (\`.wvb\`)`,
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
    const dir = this.dir ?? config.srcDir;
    if (dir == null) {
      this.logger.error(
        'Source directory is not specified. Set "srcDir" in the config file or pass [DIR] as a CLI argument.'
      );
      return 1;
    }
    const outFile = this.outFile ?? defaultOutFile(config);
    if (outFile == null) {
      this.logger.error(
        'Out file is not specified. Set "outFile" in the config file or pass "--outfile,-O" as a CLI argument.'
      );
      return 1;
    }
    const overwrite = this.overwrite ?? config.create?.overwrite ?? true;
    await create({
      dir,
      outFile,
      outDir: defaultOutDir(config),
      ignores: [this.ignores, config.create?.ignore].filter(isNotNil),
      headers: [
        config.create?.headers,
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
