import { Cli } from 'clipanion';
import { ArtifactsMergeCommand } from './commands/artifacts-merge.ts';
import { ArtifactsSpreadCommand } from './commands/artifacts-spread.ts';
import { AttwCommand } from './commands/attw.ts';
import { PrepareReleaseCommand } from './commands/prepare-release.ts';
import { Release } from './commands/release.ts';

const [, , ...args] = process.argv;

const cli = new Cli({
  binaryLabel: 'xtask',
  binaryName: 'xtask',
  binaryVersion: '0.0.0',
});

cli.register(Release);
cli.register(PrepareReleaseCommand);
cli.register(ArtifactsSpreadCommand);
cli.register(ArtifactsMergeCommand);
cli.register(AttwCommand);
cli.runExit(args);
