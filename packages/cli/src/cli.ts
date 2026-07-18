#!/usr/bin/env node
import { Cli } from 'clipanion';
// biome-ignore lint/correctness/useImportExtensions: import json file
import pkg from '../package.json' with { type: 'json' };
import { BuiltinCommand } from './commands/builtin.js';
import { DeployCommand } from './commands/deploy.js';
import { DownloadCommand } from './commands/download.js';
import { ExtractCommand } from './commands/extract.js';
import { PackCommand } from './commands/pack.js';
import { RemoteCurrentCommand } from './commands/remote/current.js';
import { RemoteListCommand } from './commands/remote/list.js';
import { RemoteLocalCommand } from './commands/remote/local.js';
import { ServeCommand } from './commands/serve.js';
import { UploadCommand } from './commands/upload.js';

const [, , ...args] = process.argv;

const cli = new Cli({
  binaryLabel: 'webview-bundle-cli',
  binaryName: 'wvb',
  binaryVersion: pkg.version,
});

cli.register(PackCommand);
cli.register(ExtractCommand);
cli.register(ServeCommand);
cli.register(UploadCommand);
cli.register(DeployCommand);
cli.register(DownloadCommand);
cli.register(RemoteCurrentCommand);
cli.register(RemoteListCommand);
cli.register(RemoteLocalCommand);
cli.register(BuiltinCommand);
cli.runExit(args);
