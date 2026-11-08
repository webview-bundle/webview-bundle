import { Cli } from 'clipanion';
import { ArtifactsMergeCommand } from './commands/artifacts-merge.ts';
import { ArtifactsSpreadCommand } from './commands/artifacts-spread.ts';
import { AttwCommand } from './commands/attw.ts';
import { PrereleaseCommand } from './commands/prerelease.ts';
import { ResolveLockfileCommand } from './commands/resolve-lockfile.ts';

const [, , ...args] = process.argv;

const cli = new Cli({
  binaryLabel: 'xtask',
  binaryName: 'xtask',
  binaryVersion: '0.0.0',
});

cli.register(PrereleaseCommand);
cli.register(ArtifactsSpreadCommand);
cli.register(ArtifactsMergeCommand);
cli.register(AttwCommand);
cli.register(ResolveLockfileCommand);
cli.runExit(args);
