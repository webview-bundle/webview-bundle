import { Remote } from '@wvb/node';
import { Command, Option } from 'clipanion';
import { resolveBundleName, resolveConfig } from '../../config.js';
import { c } from '../../console.js';
import { BaseCommand } from '../base.js';

export class RemoteCurrentCommand extends BaseCommand {
  readonly name = 'remote-current';

  static paths = [['remote', 'current']];
  static usage = Command.Usage({
    description: 'Show current Webview Bundle information from remote server.',
    details: `This command fetches and displays metadata about the currently deployed
Webview Bundle version from a remote server.

**Use Cases:**
  - Verify which version is currently active in production
  - Check bundle integrity and signature before client rollout
  - Debug deployment issues by inspecting remote state
  - Validate that a deployment completed successfully

**Displayed Information:**
  - \`Version\`: The currently deployed semantic version
  - \`ETag\`: Server-side identifier for cache validation
  - \`Integrity\`: Content hash for verification (e.g., sha384-...)
  - \`Signature\`: Cryptographic signature for authenticity
  - \`Last-Modified\`: Timestamp of the last deployment

This command only fetches metadata and does not download the bundle content.
Use \`remote download\` if you need the actual bundle file.
    `,
    examples: [
      [
        'Check current version with explicit endpoint',
        '$0 remote current my-app --endpoint https://cdn.example.com',
      ],
      ['Use bundle name and endpoint from config', '$0 remote current'],
      ['Verify deployment in CI pipeline', '$0 remote current my-app -E https://cdn.example.com'],
    ],
  });

  readonly bundleName = Option.String({
    name: 'BUNDLE',
    required: false,
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
    const endpoint = this.endpoint ?? config.remote?.endpoint;
    if (endpoint == null) {
      this.logger.error('"endpoint" is required for remote operations.');
      return 1;
    }
    const bundleName = this.bundleName ?? (await resolveBundleName(config));
    if (bundleName == null) {
      this.logger.error('"bundleName" is required for remote operations.');
      return 1;
    }
    const remote = new Remote(endpoint);
    const info = await remote.getInfo(bundleName, this.channel);
    this.logger.info(`Remote Webview Bundle info for ${c.info(bundleName)}`);
    this.logger.info(`  Version: ${c.bold(c.info(info.version))}`);
    this.logger.info(`  ETag: ${c.bold(c.info(info.etag ?? '(none)'))}`);
    this.logger.info(`  Integrity: ${c.bold(c.info(info.integrity ?? '(none)'))}`);
    this.logger.info(`  Signature: ${c.bold(c.info(info.signature ?? '(none)'))}`);
    this.logger.info(`  Last-Modified: ${c.bold(c.info(info.lastModified ?? '(none)'))}`);
  }
}
