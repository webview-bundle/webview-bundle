import path from 'node:path';
import { Command, Option } from 'clipanion';
import { isNotNil } from 'es-toolkit';
import { cascade, isBoolean, isInteger, isNumber } from 'typanion';
import { builtin } from '../api/builtin.js';
import { resolveConfig } from '../config.js';
import { BaseCommand } from './base.js';

export class BuiltinCommand extends BaseCommand {
  readonly name = 'builtin';

  static paths = [['builtin']];
  static usage = Command.Usage({
    description: 'Install builtin webview bundles from remote or local files.',
    examples: [['A basic usage', '$0 builtin']],
  });

  readonly out = Option.String('--out,-O', {
    description: 'Output directory path. [Default: "./.wvb/builtin/bundles"]',
  });
  readonly endpoint = Option.String('--endpoint,-E', {
    description: `Remote endpoint of remote server.
This option is only used when the target is "remote".`,
  });
  readonly channel = Option.String('--channel', {
    description: `Release channel to manage and distribute different stability versions. (e.g. "beta", "alpha")
This option is only used when the target is "remote".`,
  });
  readonly include = Option.Array('--include', {
    description: 'Patterns to which bundles should be included from target bundles.',
  });
  readonly exclude = Option.Array('--exclude', {
    description: 'Patterns to which bundles should be excluded from target bundles.',
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
  readonly downloadConcurrency = Option.String('--concurrency', {
    validator: cascade(isNumber(), [isInteger()]),
    description: `Concurrency of the download bundles.
This option is only used when the target is "remote".`,
  });
  readonly progress = Option.String('--progress', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: `Show download progress bar.
This option is only used when the target is "remote".
[Default: true]`,
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
    const target = config.builtin?.target ?? { type: 'remote' };

    if (target.type === 'remote') {
      target.endpoint ??= this.endpoint ?? config.remote?.endpoint;

      if (target.endpoint == null) {
        this.logger.error('Remote endpoint is required for remote target.');
        return 1;
      }
    }

    const dir = this.out ?? config.builtin?.outDir ?? path.join('.wvb', 'builtin', 'bundles');
    const include = [this.include, config.builtin?.include].filter(isNotNil);
    const exclude = [this.exclude, config.builtin?.exclude].filter(isNotNil);
    const clean = this.clean ?? config.builtin?.clean ?? true;

    await builtin({
      target,
      dir,
      include,
      exclude,
      channel: this.channel,
      clean,
      write: this.write,
      cwd: config.root,
      logLevel: this.logLevel,
      logger: this.logger,
      progress: this.progress,
    });
  }
}
