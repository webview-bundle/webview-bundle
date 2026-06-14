import { Command, Option } from 'clipanion';
import { resolveBundleName, resolveConfig, resolveVersion } from '../config.js';
import { c } from '../console.js';
import { BaseCommand } from './base.js';

export class DeployCommand extends BaseCommand {
  readonly name = 'deploy';

  static paths = [['deploy']];
  static usage = Command.Usage({
    description: 'Deploy Webview Bundle to the remote server.',
    details: `This command deploys a previously uploaded Webview Bundle version,
making it available to clients via the configured deployer.

**Channel:**
  Channels allow you to deploy different versions to different audiences.
  Common use cases include:
    - \`stable\` / \`beta\` / \`canary\` release tracks
    - \`internal\` for team testing before public release
    - A/B testing with percentage-based routing

If no channel is specified, the bundle is deployed to the default channel.
    `,
    examples: [
      ['Basic usage', '$0 remote deploy myapp'],
      ['Deploy a specific version', '$0 remote deploy myapp 1.2.0'],
      ['Deploy to a specific channel', '$0 remote deploy myapp 1.2.0 --channel beta'],
    ],
  });

  readonly bundleName = Option.String({
    name: 'BUNDLE',
    required: false,
  });
  readonly version = Option.String({
    name: 'VERSION',
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
    const bundleName =
      this.bundleName ?? (await resolveBundleName(config, config.remote?.bundleName));
    if (bundleName == null) {
      this.logger.error('"bundleName" is required for remote operations.');
      return 1;
    }
    const version = this.version ?? (await resolveVersion(config, config.remote?.version));
    if (version == null) {
      this.logger.error('Cannot get version of this Webview Bundle.');
      return 1;
    }
    if (config.remote?.deployer == null) {
      this.logger.error(
        'Cannot get "remote.deployer" from config. Make sure the "remote.deployer" is defined in config.'
      );
      return 1;
    }
    await config.remote.deployer.deploy({
      bundleName,
      version,
      channel: this.channel,
    });
    this.logger.info(`Remote Webview Bundle deployed: ${c.info(bundleName)}`);
    this.logger.info(`  Version: ${c.bold(c.info(version))}`);
    if (this.channel != null) {
      this.logger.info(`  Channel: ${c.bold(c.info(this.channel))}`);
    }
  }
}
