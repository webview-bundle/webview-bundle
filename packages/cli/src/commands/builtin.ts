import path from 'node:path';
import { Command, Option } from 'clipanion';
import { isNotNil } from 'es-toolkit';
import { cascade, isBoolean, isInteger, isNumber } from 'typanion';
import { builtin } from '../api/builtin.js';
import { resolveConfig } from '../config.js';
import {
  ANDROID_BUILTIN_OUT,
  type AndroidProject,
  checkAndroidNoCompress,
  IOS_BUILTIN_OUT,
  type IosProject,
  iosStagingDir,
  resolveAndroidProject,
  resolveIosProject,
} from '../mobile.js';
import {
  checkBundleResources,
  resolveTauriProject,
  TAURI_BUNDLES_DIR,
  type TauriProject,
} from '../tauri.js';
import { BaseCommand } from './base.js';

export class BuiltinCommand extends BaseCommand {
  readonly name = 'builtin';

  static paths = [['builtin']];
  static usage = Command.Usage({
    description: 'Install builtin webview bundles from remote or local files.',
    examples: [
      ['A basic usage', '$0 builtin'],
      ['Install into a Tauri app (wire into `beforeBundleCommand`)', '$0 builtin --tauri'],
      ['Install into the auto-detected Android app module', '$0 builtin --android'],
      ['…or an explicit Android module', '$0 builtin --android=./android/app'],
      ['Install into the auto-detected iOS project', '$0 builtin --ios'],
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
  readonly tauri = Option.Boolean('--tauri', false, {
    description: `Tauri preset. Locate the Tauri project (src-tauri), default the output to "<src-tauri>/${TAURI_BUNDLES_DIR}"
(so the runtime reads them from the Resource directory), and check that "bundle.resources" ships them.
Wire this into \`beforeBundleCommand\` (and \`beforeDevCommand\` for dev).`,
  });
  readonly tauriDir = Option.String('--tauri-dir', {
    description: `Path to the Tauri project directory (the one with tauri.conf.json). Auto-detected when omitted.
Only used together with "--tauri".`,
  });
  readonly android = Option.String('--android', {
    tolerateBoolean: true,
    description: `Android preset. Pass "--android" to auto-detect the application module (the one with "src/main/assets"),
or "--android=<module>" to point at it explicitly. Defaults the output to "<module>/${ANDROID_BUILTIN_OUT}" so the
bundles are merged into the APK/AAB assets. At runtime, extract them to a filesystem dir (assets are not filesystem paths).`,
  });
  readonly ios = Option.String('--ios', {
    tolerateBoolean: true,
    description: `iOS preset. Pass "--ios" to auto-detect the Xcode/Tuist project, or "--ios=<project>" to point at it
explicitly. Defaults the output to "<project>/${IOS_BUILTIN_OUT}". Add that directory to your Xcode target as a FOLDER
REFERENCE (not a group), regenerated from a Run Script phase above "Copy Bundle Resources".`,
  });

  async run() {
    const presetCount = [this.tauri, this.android, this.ios].filter(Boolean).length;
    if (presetCount > 1) {
      this.logger.error('Use only one of "--tauri", "--android", "--ios".');
      return 1;
    }

    // Presets auto-detect their target project (relative to --cwd, default process.cwd()). Each flag is
    // boolean-or-string: bare (`--android`) auto-detects, `--android=<path>` points at it explicitly
    // (still validated, so a wrong path yields the same clear error).
    const cwd = this.cwd ?? process.cwd();
    const androidDir = typeof this.android === 'string' ? this.android : undefined;
    const iosDir = typeof this.ios === 'string' ? this.ios : undefined;

    let tauriProject: TauriProject | null = null;
    if (this.tauri) {
      tauriProject = await resolveTauriProject(cwd, this.tauriDir);
      if (tauriProject == null) {
        this.logger.error(
          'Could not locate a Tauri project (no tauri.conf.json/json5 or Tauri.toml found). Pass "--tauri-dir <path>".'
        );
        return 1;
      }
      this.logger.info(`Tauri project: ${tauriProject.dir}`);
    }

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

    // Preset output defaults (absolute, independent of the config root):
    // - Tauri: "<src-tauri>/bundles" → lands at "<Resource>/bundles" via a `bundle.resources` glob.
    // - Android: "<module>/src/main/assets/bundles/builtin" → merged into the APK/AAB assets.
    // - iOS: the given staging dir → bundled via a folder reference.
    let defaultDir: string;
    if (tauriProject != null) {
      defaultDir = path.join(tauriProject.dir, TAURI_BUNDLES_DIR);
    } else if (androidProject != null) {
      defaultDir = path.join(androidProject.moduleDir, ANDROID_BUILTIN_OUT);
    } else if (iosProject != null) {
      defaultDir = iosStagingDir(iosProject);
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
    });

    // Post-stage guidance (warn/inform, never fail) against the platform-specific footguns.
    if (this.write !== false) {
      if (tauriProject != null) {
        // The bundler only ships what `bundle.resources` lists — warn if the bundles aren't declared.
        const status = await checkBundleResources(tauriProject.configFile);
        if (status === 'missing') {
          this.logger.warn(
            `Tauri "bundle.resources" does not reference "${TAURI_BUNDLES_DIR}", so the staged bundles won't be shipped. ` +
              `Add to ${tauriProject.configFile}:\n` +
              `  "bundle": { "resources": ["${TAURI_BUNDLES_DIR}/**/*.wvb", "${TAURI_BUNDLES_DIR}/manifest.json"] }`
          );
        } else if (status === 'skipped') {
          // TOML / JSON5 configs aren't parsed here — don't silently imply everything's fine.
          this.logger.info(
            `Could not auto-verify "bundle.resources" in ${tauriProject.configFile} (TOML/JSON5 not parsed). ` +
              `Make sure it ships "${TAURI_BUNDLES_DIR}" so the staged bundles are bundled.`
          );
        }
      } else if (androidProject != null) {
        // Already-compressed .wvb shouldn't be re-compressed in the APK.
        const status = await checkAndroidNoCompress(androidProject.moduleDir);
        if (status === 'missing') {
          this.logger.warn(
            "'.wvb' assets may be re-compressed in the APK. Add to your module's build.gradle(.kts):\n" +
              '  android { androidResources { noCompress += "wvb" } }\n' +
              'And extract the assets to a filesystem dir at runtime (assets are not filesystem paths).'
          );
        }
      } else if (iosProject != null) {
        // Copy Bundle Resources flattens groups — a folder reference is required to keep the subtree.
        this.logger.info(
          `Add "${dir}" to your Xcode target as a FOLDER REFERENCE (blue folder, not a group) and ` +
            'regenerate it from a Run Script phase placed above "Copy Bundle Resources" — otherwise the ' +
            "per-bundle subdirectories are flattened and the runtime can't find them."
        );
        if (iosProject.kind === 'tuist') {
          this.logger.info(
            `Tuist project detected: declare \`.folderReference(path: "./${iosProject.folderReferenceRoot}")\` ` +
              'in the target resources and run `tuist generate`.'
          );
        }
      }
    }
  }
}
