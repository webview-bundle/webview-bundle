import { Remote, writeBundle } from '@wvb/node';
import { Presets, SingleBar } from 'cli-progress';
import { Command, Option } from 'clipanion';
import fs from 'node:fs/promises';
import path from 'node:path';
import { isBoolean } from 'typanion';
import { logRemoteBundleInfo } from '../../api/remote/logging.js';
import { resolveConfig } from '../../config.js';
import { c } from '../../console.js';
import { formatByteLength } from '../../format.js';
import { pathExists, toAbsolutePath, withWVBExtension } from '../../fs.js';
import { BaseCommand } from '../base.js';

export class RemoteDownloadCommand extends BaseCommand {
  readonly name = 'remote-download';

  static paths = [['remote', 'download']];
  static usage = Command.Usage({
    description: 'Download Webview Bundle from remote server.',
    details: `
      This command downloads a Webview Bundle from a remote server
      and optionally saves it to disk.

      **Version Resolution:**
        - If \`VERSION\` argument is provided, that specific version is downloaded
        - Otherwise, downloads the currently deployed (latest active) version

      **Output Path:**
        - If \`--out\` is provided, the bundle is saved to that path
        - Otherwise, saves as \`<bundle-name>.wvb\` in the current directory
        - Use \`--skip-write\` to only fetch and display info without saving

      **Progress:**
        A progress bar is displayed during download for large bundles.
    `,
    examples: [
      [
        'Download latest deployed version',
        '$0 remote download my-app --endpoint https://cdn.example.com',
      ],
      [
        'Download a specific version',
        '$0 remote download my-app 1.2.0 --endpoint https://cdn.example.com',
      ],
      [
        'Download and save to a custom path',
        '$0 remote download my-app --out ./bundles/my-app.wvb',
      ],
      ['Overwrite existing file', '$0 remote download my-app --out ./my-bundle.wvb --overwrite'],
      ['Fetch info only without saving', '$0 remote download my-app --skip-write'],
    ],
  });

  readonly bundleName = Option.String({
    name: 'BUNDLE',
    required: false,
  });
  readonly version = Option.String({
    name: 'VERSION',
  });
  readonly out = Option.String('--out,-O', {
    description: 'Output file path.',
  });
  readonly endpoint = Option.String('--endpoint,-E', {
    description: 'Endpoint of remote server.',
  });
  readonly channel = Option.String('--channel', {
    description:
      'Release channel to manage and distribute different stability versions. (e.g. "beta", "alpha")',
  });
  readonly write = Option.String('--write', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: `Writing files on disk.
Set this to \`false\` (or pass "--no-write") just for simulating operation.
[Default: true]`,
  });
  readonly overwrite = Option.String('--overwrite', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Overwrite outfile if file is already exists. [Default: false]',
  });
  readonly progress = Option.String('--progress', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Show download progress bar. [Default: true]',
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
    const bundleName = this.bundleName ?? config.remote?.bundleName;
    if (bundleName == null) {
      this.logger.error('"bundleName" is required for remote operations.');
      return 1;
    }
    const progress = this.progress
      ? new SingleBar(
          {
            format: `{bundleName} ${c.progress('{bar}')} {percentage}% ({value}/{total})`,
            clearOnComplete: false,
            // https://github.com/npkgz/cli-progress/issues/126
            gracefulExit: false,
          },
          Presets.shades_classic
        )
      : null;
    const remote = new Remote(endpoint, {
      onDownload: data => {
        if (progress?.isActive !== true) {
          progress?.start(data.totalBytes, data.downloadedBytes);
        } else {
          progress?.update(data.downloadedBytes);
        }
      },
    });
    const [info, bundle, buf] =
      this.version != null
        ? await remote.downloadVersion(bundleName, this.version)
        : await remote.download(bundleName, this.channel);
    logRemoteBundleInfo(this.logger, info, buf.byteLength);

    const write = this.write ?? true;
    if (!write) {
      return 0;
    }

    const outFile = this.out ?? withWVBExtension(bundleName);
    const outFilePath = toAbsolutePath(outFile, config.root);
    if (await pathExists(outFilePath)) {
      const overwrite = this.overwrite ?? false;
      if (!overwrite) {
        this.logger.error(`File already exists: ${outFile}`);
        return 1;
      }
    }
    await fs.mkdir(path.dirname(outFilePath), { recursive: true });
    const size = await writeBundle(bundle, outFilePath);
    this.logger.info(
      `Output: ${c.bold(c.success(outFile))} ${c.bytes(formatByteLength(Number(size)))}`
    );
  }
}
