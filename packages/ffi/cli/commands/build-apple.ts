import fs from 'node:fs/promises';
import path from 'node:path';
import { Command } from 'clipanion';
import { glob } from 'tinyglobby';
import { LIB_NAME, PKG_NAME } from '../cargo.ts';
import { PKG_DIR, ROOT_DIR } from '../consts.ts';
import { getProfileTargetDir, type Profile, ProfileOption } from '../profile.ts';
import { runCommand } from '../run.ts';
import {
  type ApplePlatform,
  type AppleTarget,
  AppleTargetOption,
  AppleTargetSchema,
  getApplePlatformFromTarget,
} from '../target.ts';
import { generateUniffiBindings } from '../uniffi.ts';
import { zip } from '../zip.ts';

export class BuildAppleCommand extends Command {
  static paths = [['build', 'apple']];

  readonly profile = ProfileOption;
  readonly targets = AppleTargetOption;

  async execute() {
    const targets = AppleTargetSchema.array()
      .nonempty()
      .parse(this.targets ?? AppleTargetSchema.options);

    const appleDir = path.join(PKG_DIR, 'apple');

    const headersDir = path.join(appleDir, 'headers');
    const swiftDir = path.join(appleDir, 'src');
    const tmpDir = path.join(appleDir, '.uniffi-tmp');
    const outputDir = path.join(PKG_DIR, '.output');

    await Promise.all(
      [headersDir, swiftDir, tmpDir].map(dir => fs.rm(dir, { recursive: true, force: true }))
    );

    const buildOutputs = new Map<ApplePlatform, string[]>();
    for (const target of targets) {
      const libPath = await this.build(target, this.profile);
      const platform = getApplePlatformFromTarget(target);

      buildOutputs.has(platform)
        ? buildOutputs.get(platform)!.push(libPath)
        : buildOutputs.set(platform, [libPath]);
    }

    const libPaths: string[] = [];
    for (const [platform, platformLibPaths] of buildOutputs.entries()) {
      if (platformLibPaths.length === 1) {
        libPaths.push(...platformLibPaths);
      } else {
        const libPath = await this.lipo(platform, platformLibPaths, appleDir);
        libPaths.push(libPath);
      }
    }

    console.log('built libs:');
    for (const libPath of libPaths) {
      console.log(`- ${path.relative(ROOT_DIR, libPath)}`);
    }

    await generateUniffiBindings('swift', libPaths[0]!, tmpDir);
    await this.moveFiles('*.h', tmpDir, headersDir);
    await this.consolidateModulemapFiles(tmpDir, headersDir);
    await this.moveFiles(['*.swift', '!Package.swift'], tmpDir, swiftDir);
    await fs.rm(tmpDir, { recursive: true, force: true });

    const xcframeworkPath = path.join(appleDir, `WebViewBundleFFI.xcframework`);
    await this.createXCFramework(libPaths, headersDir, xcframeworkPath);
    console.log(`xcframework: ${path.relative(ROOT_DIR, xcframeworkPath)}`);

    await fs.rm(headersDir, { recursive: true });
    await this.zip(appleDir, outputDir);
  }

  private async build(target: AppleTarget, profile: Profile) {
    const args = ['build', '-p', PKG_NAME, '--target', target, '--profile', profile];
    await runCommand('cargo', args, {
      cwd: ROOT_DIR,
      prefix: `[build:${target}]`,
    });

    const libPath = path.join(getProfileTargetDir(profile, target), `lib${LIB_NAME}.a`);
    return libPath;
  }

  private async lipo(platform: ApplePlatform, libPaths: string[], appleDir: string) {
    const outputDir = path.join(appleDir, 'lipo', platform);

    await fs.rm(outputDir, { recursive: true, force: true });
    await fs.mkdir(outputDir, { recursive: true });

    const output = path.join(outputDir, `lib${LIB_NAME}.a`);

    console.log(`lipo(${platform}): ${path.relative(ROOT_DIR, output)}`);
    for (const libPath of libPaths) {
      console.log(`- ${path.relative(ROOT_DIR, libPath)}`);
    }

    await runCommand('lipo', ['-create', ...libPaths, '-output', output], {
      cwd: ROOT_DIR,
      prefix: `[lipo:${platform}]`,
    });

    return output;
  }

  private async moveFiles(patterns: string | string[], srcDir: string, destDir: string) {
    const files = await glob(patterns, {
      cwd: srcDir,
      onlyFiles: true,
    });
    for (const file of files) {
      const destFile = path.join(destDir, file);

      await fs.mkdir(path.dirname(destFile), { recursive: true });
      await fs.rename(path.join(srcDir, file), destFile);
    }
  }

  private async consolidateModulemapFiles(srcDir: string, destDir: string) {
    const files = await glob('*.modulemap', {
      cwd: srcDir,
      onlyFiles: true,
    });

    if (files.length === 0) {
      return;
    }

    const outputPath = path.join(destDir, 'module.modulemap');

    if (files.length === 1) {
      await fs.rename(path.join(srcDir, files[0]!), outputPath);
      return;
    }

    // Merge multiple modulemap files: use first as base, append header
    // declarations from remaining files
    const [base, ...rest] = files as [string, ...string[]];
    let baseContent = await fs.readFile(path.join(srcDir, base), 'utf-8');

    const extraHeaders: string[] = [];
    for (const file of rest) {
      const content = await fs.readFile(path.join(srcDir, file), 'utf-8');
      const headerMatches = content.matchAll(/^\s*header\s+"[^"]+"/gm);
      for (const match of headerMatches) {
        extraHeaders.push(match[0]!.trim());
      }
    }

    if (extraHeaders.length > 0) {
      // Insert extra headers after the first header declaration
      baseContent = baseContent.replace(
        /(^\s*header\s+"[^"]+")/m,
        `$1\n  ${extraHeaders.join('\n  ')}`
      );
    }

    await fs.writeFile(outputPath, baseContent);
  }

  private async createXCFramework(libPaths: string[], headersDir: string, outputPath: string) {
    await fs.rm(outputPath, { recursive: true, force: true });

    const args: string[] = ['-create-xcframework'];
    for (const libPath of libPaths) {
      args.push('-library', libPath, '-headers', headersDir);
    }
    args.push('-output', outputPath);

    await runCommand('xcodebuild', args, {
      cwd: ROOT_DIR,
      prefix: '[xcframework] ',
    });
  }

  private async zip(appleDir: string, outputDir: string) {
    await fs.mkdir(outputDir, { recursive: true });

    const appleZip = path.join(outputDir, 'apple.zip');
    await fs.rm(appleZip, { force: true });
    await zip(appleZip, appleDir, ['lipo/**/*', 'src/**/*', 'Sources/**/*']);

    const xcframeworkZip = path.join(outputDir, 'WebViewBundleFFI.xcframework.zip');
    await fs.rm(xcframeworkZip, { force: true });
    await zip(xcframeworkZip, appleDir, ['WebViewBundleFFI.xcframework/**/*']);
  }
}
