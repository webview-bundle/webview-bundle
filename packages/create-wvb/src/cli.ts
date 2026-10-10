#!/usr/bin/env node
import { Builtins, Cli } from 'clipanion';
// biome-ignore lint/correctness/useImportExtensions: import json file
import pkg from '../package.json' with { type: 'json' };
import { CreateCommand } from './command.js';

const [, , ...args] = process.argv;

const cli = new Cli({
  binaryLabel: 'create-wvb',
  binaryName: 'create-wvb',
  binaryVersion: pkg.version,
});

cli.register(CreateCommand);
cli.register(Builtins.HelpCommand);
cli.register(Builtins.VersionCommand);
cli.runExit(args);
