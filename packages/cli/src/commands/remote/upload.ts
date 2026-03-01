import { Command, Option } from 'clipanion';
import path from 'node:path';
import { isBoolean } from 'typanion';
import { remoteUpload } from '../../api/remote/upload.js';
import { defaultOutDir, defaultOutFile, resolveConfig } from '../../config.js';
import { c } from '../../console.js';
import { buildURL } from '../../utils/url.js';
import { BaseCommand } from '../base.js';

export class RemoteUploadCommand extends BaseCommand {
  readonly name = 'remote-upload';

  static paths = [['remote', 'upload']];
  static usage = Command.Usage({
    description: 'Upload Webview Bundle to remote server.',
    details: `
This command uploads a built Webview Bundle (.wvb) to a remote server.

The upload process includes:
1. Reading the bundle file from disk
2. Computing integrity hash (optional, configurable)
3. Signing the bundle with a cryptographic signature (optional, configurable)
4. Uploading to the remote server via the configured uploader

Bundle Resolution:
- If \`--file\` is provided, that file is used directly
- Otherwise, uses the \`outFile\` from your config
- Bundle name defaults to the filename (without .wvb extension)

Version Resolution:
- If \`VERSION\` argument is provided, that version is used
- Otherwise, falls back to \`version\` field in package.json

Integrity & Signature:
Integrity and signature ensure bundle authenticity at runtime.
Configure these in your \`wvb.config.ts\` under \`remote.integrity\` 
and \`remote.signature\`. Use \`--skip-integrity\` or \`--skip-signature\`
to bypass these steps when needed.
    `,
    examples: [
      ['Upload bundle with auto-detected settings', '$0 remote upload'],
      ['Upload a specific bundle file', '$0 remote upload --file ./dist/myapp.wvb'],
      ['Upload with explicit name and version', '$0 remote upload myapp 1.2.0'],
      ['Force overwrite existing version', '$0 remote upload myapp 1.2.0 --force'],
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
        'Cannot get "uploader" from remote config. Make sure the "uploader" is defined in remote config.'
      );
      return 1;
    }
    const defaultFile = defaultOutFile(config);
    const file =
      this.file ??
      (defaultFile != null ? path.join(defaultOutDir(config), defaultFile) : undefined);
    if (file == null) {
      this.logger.error(
        'Webview Bundle file is not specified. Set "outFile" in the config file ' +
          'or pass "--file,-F" as a CLI argument.'
      );
      return 1;
    }
    const version = this.version ?? config.packageJson?.version;
    if (version == null) {
      this.logger.error('Cannot get version of this Webview Bundle.');
      return 1;
    }
    const bundleName = this.bundleName ?? config.remote?.bundleName;

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

    const dest =
      config.remote.endpoint != null
        ? buildURL(config.remote.endpoint, `/bundles/${bundleName}`).toString()
        : null;
    if (dest != null) {
      this.logger.info(`  Bundle Endpoint: ${c.bold(c.info(dest))}`);
    }
  }
}
