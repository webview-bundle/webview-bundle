import { Command, Option } from 'clipanion';
import { isBoolean } from 'typanion';
import { unpack } from '../api/unpack.js';
import { BaseCommand } from './base.js';

export class UnpackCommand extends BaseCommand {
  readonly name = 'unpack';
  static paths = [['unpack']];
  static usage = Command.Usage({
    description: 'Unpack webview bundle files.',
    examples: [
      ['A basic usage', '$0 unpack ./dist.wvb'],
      ['Specify outdir path', '$0 unpack ./dist.wvb --outdir ./dist'],
    ],
  });

  readonly file = Option.String({
    name: 'FILE',
    required: true,
  });
  readonly outDir = Option.String('--outdir,-O', {
    description: `Outdir path to unpack webview bundle files.
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
    description: 'Clean up unpack files if out directory already exists. [Default: false]',
  });
  readonly cwd = Option.String('--cwd', {
    description: 'Set the working directory for resolving paths. [Default: process.cwd()]',
  });

  async run() {
    await unpack({
      file: this.file,
      outDir: this.outDir,
      cwd: this.cwd,
      write: this.write,
      clean: this.clean,
      logger: this.logger,
    });
  }
}
