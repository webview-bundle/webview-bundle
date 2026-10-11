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
import { withWvbExtension } from '../fs.js';
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
      ['Upload with explicit name and version', '$0 upload myapp --version=1.2.0'],
    ],
  });

  readonly bundleName = Option.String({
    name: 'BUNDLE',
    required: false,
  });
  readonly version = Option.String('--version,-V', {
    description: 'Version of the bundle to deploy.',
  });
  readonly file = Option.String('--file,-F', {
    description: 'Path to the Webview Bundle file (.wvb) to upload.',
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
      integrity: this.skipIntegrity ? false : config.remote?.integrity,
      cwd: config.root,
      logger: this.logger,
    });
  }

  private resolveFile(config: ResolvedConfig): string | undefined {
    if (this.file != null) {
      return withWvbExtension(this.file);
    }
    const outFile = resolveOutFile(config);
    return outFile != null ? withWvbExtension(outFile) : undefined;
  }
}
