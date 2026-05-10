import { Command, Option } from 'clipanion';
import { isBoolean } from 'typanion';
import { extract } from '../api/extract.js';
import { BaseCommand } from './base.js';

export class ExtractCommand extends BaseCommand {
  readonly name = 'extract';
  static paths = [['extract']];
  static usage = Command.Usage({
    description: 'Extract webview bundle files.',
    examples: [
      ['A basic usage', '$0 extract ./dist.wvb'],
      ['Specify outdir path', '$0 extract ./dist.wvb --outdir ./dist'],
    ],
  });

  readonly file = Option.String({
    name: 'FILE',
    required: true,
  });
  readonly outDir = Option.String('--outdir,-O', {
    description: `Outdir path to extract webview bundle files.
If not provided, will use webview bundle file name as directory with based on \`.wvb\` directory.`,
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
    description: 'Clean up extracted files if out directory already exists. [Default: false]',
  });
  readonly cwd = Option.String('--cwd', {
    description: 'Set the working directory for resolving paths. [Default: process.cwd()]',
  });

  async run() {
    await extract({
      file: this.file,
      outDir: this.outDir,
      cwd: this.cwd,
      write: this.write,
      clean: this.clean,
      logger: this.logger,
    });
  }
}
