import { Remote } from '@wvb/node';
import { Command, Option } from 'clipanion';
import { resolveConfig } from '../../config.js';
import { BaseCommand } from '../base.js';

export class RemoteListCommand extends BaseCommand {
  readonly name = 'remote-list';

  static paths = [
    ['remote', 'list'],
    ['remote', 'ls'],
  ];
  static usage = Command.Usage({
    description: 'List all Webview Bundles available on remote server.',
    details: `This command retrieves and displays a list of all Webview Bundles
stored on the remote server.

**Use Cases:**
  - Discover available bundles on a remote server
  - Audit deployed bundles across environments
  - Find bundle names for use with other commands
  - Inventory check before cleanup or migration

**Output Format:**
  The bundle list is displayed as JSON, making it easy to:
    - Parse in scripts and CI pipelines
    - Pipe to tools like \`jq\` for filtering
    - Integrate with monitoring and alerting systems

**Aliases:**
  This command can be invoked as either \`remote list\` or \`remote ls\`.
    `,
    examples: [
      ['List all bundles on a server', '$0 remote ls --endpoint https://cdn.example.com'],
      ['List bundles using endpoint from config', '$0 remote list'],
      ['List and filter with jq', '$0 remote ls -E https://cdn.example.com | jq ".[].name"'],
    ],
  });

  readonly endpoint = Option.String('--endpoint,-E', {
    description: 'Endpoint of remote server.',
  });
  readonly channel = Option.String('--channel', {
    description:
      'Release channel to manage and distribute different stability versions. (e.g. "beta", "alpha")',
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
    const endpointInput = this.endpoint ?? config.remote?.endpoint;
    if (endpointInput == null) {
      this.logger.error('"endpoint" is required for remote operations.');
      return 1;
    }
    const remote = new Remote(endpointInput);
    const bundles = await remote.listBundles(this.channel);
    this.logger.info(`Remote Webview Bundles:`);
    console.log(JSON.stringify(bundles, null, 2));
  }
}
