import { Command, Option } from 'clipanion';
import { isNotNil } from 'es-toolkit';
import path from 'node:path';
import { cascade, isBoolean, isInteger, isNumber } from 'typanion';
import { builtin } from '../api/builtin.js';
import { resolveConfig } from '../config.js';
import { BaseCommand } from './base.js';

export class BuiltinCommand extends BaseCommand {
  readonly name = 'builtin';

  static paths = [['builtin']];
  static usage = Command.Usage({
    description: 'Install builtin webview bundles from remote.',
    examples: [['A basic usage', '$0 builtin']],
  });

  readonly out = Option.String('--out,-O', {
    description: 'Output directory path.',
  });
  readonly endpoint = Option.String('--endpoint,-E', {
    description: 'Endpoint of remote server.',
  });
  readonly channel = Option.String('--channel', {
    description:
      'Release channel to manage and distribute different stability versions. (e.g. "beta", "alpha")',
  });
  readonly include = Option.Array('--include', {
    description: 'Patterns to which bundles should be included from remote bundles.',
  });
  readonly exclude = Option.Array('--exclude', {
    description: 'Patterns to which bundles should be excluded from remote bundles.',
  });
  readonly write = Option.String('--write', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: `Writing files on disk.
Set this to \`false\` (or pass "--no-write") just for simulating operation.
[Default: true]`,
  });
  readonly clean = Option.String('--clean', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Clean up builtin directory before the operation. [Default: true]',
  });
  readonly concurrency = Option.String('--concurrency', {
    validator: cascade(isNumber(), [isInteger()]),
    description: 'Concurrency of the download bundles.',
  });
  readonly progress = Option.String('--progress', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Show download progress bar. [Default: true]',
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
    const endpoint = this.endpoint ?? config.remote?.endpoint;
    if (endpoint == null) {
      this.logger.error('"endpoint" is required for remote operations.');
      return 1;
    }
    const dir = this.out ?? config.builtin?.outDir ?? path.join(config.outDir ?? '.wvb', 'builtin');
    const include = [this.include, config.builtin?.include].filter(isNotNil);
    const exclude = [this.exclude, config.builtin?.exclude].filter(isNotNil);
    const clean = this.clean ?? config.builtin?.clean ?? true;
    await builtin({
      remoteEndpoint: endpoint,
      dir,
      include,
      exclude,
      channel: this.channel,
      clean,
      write: this.write,
      cwd: config.root,
      logger: this.logger,
      concurrency: this.concurrency,
      progress: this.progress,
    });
  }
}
