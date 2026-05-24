import { Cli } from 'clipanion';
import { BuildAndroidCommand } from './commands/build-android.ts';
import { BuildAppleCommand } from './commands/build-apple.ts';
import { TestAndroidCommand } from './commands/test-android.ts';
import { TestAppleCommand } from './commands/test-apple.ts';

const [, , ...args] = process.argv;

const cli = new Cli({
  binaryLabel: 'ffi-cli',
  binaryName: 'ffi-cli',
  binaryVersion: '0.0.0',
});

cli.register(BuildAndroidCommand);
cli.register(BuildAppleCommand);
cli.register(TestAndroidCommand);
cli.register(TestAppleCommand);
cli.runExit(args);
