import { Command, Option } from 'clipanion';
import { cascade, isBoolean, isInExclusiveRange, isInteger, isNumber } from 'typanion';
import { localRemote } from '../../api/index.js';
import { BaseCommand } from '../base.js';

export class RemoteLocalCommand extends BaseCommand {
  readonly name = 'remote-local';

  static paths = [['remote', 'local']];
  static usage = Command.Usage({
    description: 'Start a local remote server for development',
    examples: [
      ['Basic usage', '$ remote local'],
      ['Specify base dir', '$ remote local --base-dir ./.wvb/local'],
    ],
  });

  readonly baseDir = Option.String('--base-dir', {
    description: 'Specify a base directory for the local remote server. [Default: ~/.wvb/local]',
  });
  readonly allowOtherVersions = Option.String('--allow-other-versions', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Allow other versions to be served. [Default: false]',
  });
  readonly hostname = Option.String('--hostname,-H', {
    description: 'Specify a hostname on which to start the http server. [Default: localhost]',
    env: 'HOSTNAME',
  });
  readonly port = Option.String('--port,-P', '4313', {
    description:
      'Specify a port number on which to start the http server. [Default: 4313] [env: PORT]',
    validator: cascade(isNumber(), [isInteger(), isInExclusiveRange(1, 65535)]),
    env: 'PORT',
  });

  async run() {
    const port = this.port ?? 4313;
    const instance = await localRemote({
      baseDir: this.baseDir,
      hostname: this.hostname,
      port,
      allowOtherVersions: this.allowOtherVersions,
      logger: this.logger,
    });
    const handleShutdown = () => {
      instance
        .shutdown()
        .then(() => process.exit(0))
        .catch(() => process.exit(1));
    };
    process.on('SIGINT', handleShutdown);
    process.on('SIGTERM', handleShutdown);
  }
}
