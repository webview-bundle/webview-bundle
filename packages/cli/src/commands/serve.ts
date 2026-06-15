import { Command, Option } from 'clipanion';
import { cascade, isBoolean, isInExclusiveRange, isInteger, isNumber } from 'typanion';
import { serve } from '../api/serve.js';
import { resolveConfig, resolveOutFile } from '../config.js';
import { isColorEnabled } from '../console.js';
import { BaseCommand } from './base.js';

export class ServeCommand extends BaseCommand {
  readonly name = 'serve';
  static paths = [['serve']];
  static usage = Command.Usage({
    description: 'Serve webview bundle files with localhost server.',
    examples: [
      ['A basic usage', '$0 serve ./dist.wvb'],
      ['Start server with hostname', '$0 serve ./dist.wvb --hostname 0.0.0.0'],
      ['Specify server port', '$0 serve ./dist.wvb --port 4312'],
    ],
  });

  readonly file = Option.String({
    name: 'FILE',
    required: false,
  });
  readonly hostname = Option.String('--hostname,-H', {
    description: 'Specify a hostname on which to start the http server. [Default: localhost]',
    env: 'HOSTNAME',
  });
  readonly port = Option.String('--port,-P', '4312', {
    description:
      'Specify a port number on which to start the http server. [Default: 4312] [env: PORT]',
    validator: cascade(isNumber(), [isInteger(), isInExclusiveRange(1, 65535)]),
    env: 'PORT',
  });
  readonly silent = Option.String('--silent', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Disable middleware log output.',
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
    const file = this.file ?? config.serve?.file ?? resolveOutFile(config);
    if (file == null) {
      this.logger.error(
        'Webview Bundle file is not specified. Set "serve.file" (or "pack.outFile") in the config file ' +
          'or pass [FILE] as a CLI argument.'
      );
      return 1;
    }
    const silent = this.silent ?? config.serve?.silent ?? false;
    const port = this.port ?? config.serve?.port ?? 4312;
    const instance = await serve({
      file,
      hostname: this.hostname,
      port,
      silent,
      cwd: config.root,
      logger: this.logger,
      colorEnabled: isColorEnabled(),
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
