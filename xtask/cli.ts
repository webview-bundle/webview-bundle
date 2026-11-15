import { Cli } from 'clipanion';
import { AttwCommand } from './commands/attw.ts';
import { ResolveLockfileCommand } from './commands/resolve-lockfile.ts';
import { SizeCollectCommand, SizeCompareCommand } from './commands/size.ts';

const [, , ...args] = process.argv;

const cli = new Cli({
  binaryLabel: 'xtask',
  binaryName: 'xtask',
  binaryVersion: '0.0.0',
});

cli.register(AttwCommand);
cli.register(ResolveLockfileCommand);
cli.register(SizeCollectCommand);
cli.register(SizeCompareCommand);
cli.runExit(args);
