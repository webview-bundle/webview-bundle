import path from 'node:path';
import { Command, Option } from 'clipanion';
import { isBoolean } from 'typanion';
import { pack } from '../api/pack.js';
import { remoteUpload } from '../api/upload.js';
import {
  type ResolvedConfig,
  resolveBundleName,
  resolveConfig,
  resolveOutFile,
  resolveVersion,
} from '../config.js';
import { c } from '../console.js';
import { withWvbExtension } from '../fs.js';
import { buildURL } from '../utils/url.js';
import { BaseCommand } from './base.js';

export class UploadCommand extends BaseCommand {
  readonly name = 'upload';

  static paths = [['upload']];
  static usage = Command.Usage({
    description: 'Upload Webview Bundle to remote server.',
    details: `
This command uploads a built Webview Bundle (.wvb) to a remote server.

The upload process includes:
1. Pack webview bundle archive from disk files
2. Computing integrity hash (optional, configurable)
3. Signing the bundle with a cryptographic signature (optional, configurable)
4. Uploading to the remote server via the configured uploader`,
    examples: [
      ['Basic usage', '$0 remote upload'],
      ['Upload a specific bundle file', '$0 upload --file ./dist/myapp.wvb'],
      ['Upload with explicit name and version', '$0 upload myapp 1.2.0'],
      ['Force overwrite existing version', '$0 upload myapp 1.2.0 --force'],
      ['Deploy after upload is done', '$0 upload --deploy'],
    ],
  });

  readonly bundleName = Option.String({
    name: 'BUNDLE',
    required: false,
  });
  readonly version = Option.String({
    name: 'VERSION',
    required: false,
  });
  readonly file = Option.String('--file,-F', {
    description: 'Path to the Webview Bundle file (.wvb) to upload.',
  });
  readonly force = Option.String('--force', false, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Overwrite if the same version already exists on remote.',
  });
  readonly deploy = Option.String('--deploy', false, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Deploy the bundle to the remote endpoint after upload.',
  });
  readonly channel = Option.String('--channel', {
    description: `Release channel to manage and distribute different stability versions. (e.g. "beta", "alpha")
This option can be used when the deploy options is enabled.`,
  });
  readonly pack = Option.String('--pack,-P', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Pack the bundle before upload. [Default: true]',
  });
  readonly skipIntegrity = Option.String('--skip-integrity', false, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Skip computing integrity hash for the bundle.',
  });
  readonly skipSignature = Option.String('--skip-signature', false, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Skip signing the bundle with a cryptographic signature.',
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
    if (config.remote?.uploader == null) {
      this.logger.error(
        'Cannot get "remote.uploader" from config. Make sure the "remote.uploader" is defined in config.'
      );
      return 1;
    }
    if (this.deploy && config.remote?.deployer == null) {
      this.logger.error(
        'Deploy is enabled but cannot get "remote.deployer" from config ' +
          'Make sure the "remote.deployer" is defined in config.'
      );
      return 1;
    }

    const file = this.resolveFile(config);
    if (file == null) {
      this.logger.error(
        'Webview Bundle file is not specified. Set "pack.outFile" in the config file ' +
          'or pass "--file,-F" as a CLI argument.'
      );
      return 1;
    }

    const packBeforeUpload = this.pack ?? config.remote?.packBeforeUpload ?? true;
    if (packBeforeUpload) {
      const srcDir = config.pack?.srcDir ?? './dist';
      const overwrite = config.pack?.overwrite ?? true;
      await pack({
        srcDir,
        outFile: file,
        overwrite,
        write: true,
        cwd: config.root,
        logLevel: this.logLevel,
        logger: this.logger,
      });
    }

    const version = this.version ?? (await resolveVersion(config, config.remote?.version));
    if (version == null) {
      this.logger.error('Cannot get version of this Webview Bundle.');
      return 1;
    }

    const bundleName =
      this.bundleName ??
      (await resolveBundleName(config, config.remote?.bundleName, { file })) ??
      path.basename(file, '.wvb');

    await remoteUpload({
      file,
      bundleName,
      version,
      uploader: config.remote.uploader,
      force: this.force,
      integrity: this.skipIntegrity ? false : config.remote?.integrity,
      signature: this.skipSignature ? undefined : config.remote?.signature,
      cwd: config.root,
      logger: this.logger,
    });

    if (this.deploy) {
      await config.remote.deployer!.deploy({
        bundleName,
        version,
        channel: this.channel,
      });
    }

    const dest =
      config.remote.endpoint != null
        ? buildURL(config.remote.endpoint, `/bundles/${bundleName}`).toString()
        : null;
    if (dest != null) {
      this.logger.info(`  Bundle Endpoint: ${c.bold(c.info(dest))}`);
    }
  }

  private resolveFile(config: ResolvedConfig): string | undefined {
    if (this.file != null) {
      return withWvbExtension(this.file);
    }
    const outFile = resolveOutFile(config);
    return outFile != null ? withWvbExtension(outFile) : undefined;
  }
}
