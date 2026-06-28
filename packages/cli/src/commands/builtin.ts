import path from 'node:path';
import { Command, Option } from 'clipanion';
import { isNotNil } from 'es-toolkit';
import { cascade, isBoolean, isInteger, isNumber } from 'typanion';
import { builtin } from '../api/builtin.js';
import { resolveConfig } from '../config.js';
import {
  type AndroidProject,
  defaultAndroidBundlesDir,
  defaultIosProjectBundlesDir,
  type IosProject,
  resolveAndroidProject,
  resolveIosProject,
} from '../mobile.js';
import { BaseCommand } from './base.js';

export class BuiltinCommand extends BaseCommand {
  readonly name = 'builtin';

  static paths = [['builtin']];
  static usage = Command.Usage({
    description: 'Install builtin webview bundles from remote or local files.',
    examples: [
      ['A basic usage', '$0 builtin'],
      ['Install into the auto-detected Android app module', '$0 builtin --android'],
      [
        '…or an explicit Android module (where build.gradle(.kts) lives)',
        '$0 builtin --android=./testapp',
      ],
      ['Install into the auto-detected iOS project', '$0 builtin --ios'],
      ['…or an explicit iOS project', '$0 builtin --ios=./testapp'],
    ],
  });

  readonly out = Option.String('--out,-O', {
    description: 'Output directory path. [Default: "./.wvb/builtin/bundles"]',
  });
  readonly endpoint = Option.String('--endpoint,-E', {
    description: `Remote endpoint of remote server.
This option is only used when the target is "remote".`,
  });
  readonly channel = Option.String('--channel', {
    description: `Release channel to manage and distribute different stability versions. (e.g. "beta", "alpha")
This option is only used when the target is "remote".`,
  });
  readonly include = Option.Array('--include', {
    description: 'Patterns to which bundles should be included from target bundles.',
  });
  readonly exclude = Option.Array('--exclude', {
    description: 'Patterns to which bundles should be excluded from target bundles.',
  });
  readonly write = Option.String('--write', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: `Writing files on disk.
Set this to \`false\` (or pass "--no-write") just for simulating operation.
[Default: true]`,
  });
  readonly clean = Option.String('--clean', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Clean up builtin directory before the operation. [Default: true]',
  });
  readonly downloadConcurrency = Option.String('--concurrency', {
    validator: cascade(isNumber(), [isInteger()]),
    description: `Concurrency of the download bundles.
This option is only used when the target is "remote".`,
  });
  readonly progress = Option.String('--progress', true, {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: `Show download progress bar.
This option is only used when the target is "remote".
[Default: true]`,
  });
  readonly configFile = Option.String('--config,-C', {
    description: 'Path to the config file.',
  });
  readonly cwd = Option.String('--cwd', {
    description: 'Set the working directory for resolving paths. [Default: process.cwd()]',
  });
  readonly android = Option.String('--android', {
    tolerateBoolean: true,
    description: `Android preset. Pass "--android" to auto-detect the application module, or "--android=<module>" to point at
the module directory (where "build.gradle(.kts)" lives).`,
  });
  readonly ios = Option.String('--ios', {
    tolerateBoolean: true,
    description: `iOS preset. Pass "--ios" to auto-detect the project, or "--ios=<project>" to point at it explicitly.
This will adds a \`folderReference\` to Project.swift.`,
  });

  async run() {
    const presetCount = [this.android, this.ios].filter(Boolean).length;
    if (presetCount > 1) {
      this.logger.error('Use only one of "--android", "--ios".');
      return 1;
    }

    // Presets auto-detect their target project (relative to --cwd, default process.cwd()). Each flag is
    // boolean-or-string: bare (`--android`) auto-detects, `--android=<path>` points at it explicitly
    // (still validated, so a wrong path yields the same clear error).
    const cwd = this.cwd ?? process.cwd();
    const androidDir = typeof this.android === 'string' ? this.android : undefined;
    const iosDir = typeof this.ios === 'string' ? this.ios : undefined;

    let androidProject: AndroidProject | null = null;
    if (this.android) {
      androidProject = await resolveAndroidProject(cwd, androidDir);
      if (androidProject == null) {
        this.logger.error(
          'Could not locate an Android application module (no com.android.application module with ' +
            'src/main/assets found). Pass "--android=<path>".'
        );
        return 1;
      }
      this.logger.info(`Android app module: ${androidProject.moduleDir}`);
    }

    let iosProject: IosProject | null = null;
    if (this.ios) {
      iosProject = await resolveIosProject(cwd, iosDir);
      if (iosProject == null) {
        this.logger.error(
          'Could not locate an iOS project (no Project.swift, *.xcworkspace, or *.xcodeproj found). ' +
            'Pass "--ios=<path>".'
        );
        return 1;
      }
      this.logger.info(`iOS project: ${iosProject.dir}`);
    }

    const config = await resolveConfig({
      root: this.cwd,
      configFile: this.configFile,
    });
    const target = config.builtin?.target ?? { type: 'remote' };

    if (target.type === 'remote') {
      target.endpoint ??= this.endpoint ?? config.remote?.endpoint;

      if (target.endpoint == null) {
        this.logger.error('Remote endpoint is required for remote target.');
        return 1;
      }

      if (this.downloadConcurrency != null) {
        target.download ??= {};
        target.download.concurrency = this.downloadConcurrency;
      }
    }

    let defaultDir: string;
    if (androidProject != null) {
      defaultDir = defaultAndroidBundlesDir(androidProject);
    } else if (iosProject != null) {
      defaultDir = defaultIosProjectBundlesDir(iosProject);
    } else {
      defaultDir = config.builtin?.outDir ?? path.join('.wvb', 'builtin', 'bundles');
    }
    const dir = this.out ?? defaultDir;
    const include = [this.include, config.builtin?.include].filter(isNotNil);
    const exclude = [this.exclude, config.builtin?.exclude].filter(isNotNil);
    const clean = this.clean ?? config.builtin?.clean ?? true;

    await builtin({
      target,
      dir,
      include,
      exclude,
      channel: this.channel,
      clean,
      write: this.write,
      cwd: config.root,
      logLevel: this.logLevel,
      logger: this.logger,
      progress: this.progress,
      android:
        this.write && androidProject != null
          ? {
              dir: androidProject.moduleDir,
              checkNoCompress: true,
            }
          : undefined,
      ios:
        this.write && iosProject != null
          ? {
              dir: iosProject.dir,
              addProjectFolderReference: true,
            }
          : undefined,
    });
  }
}
