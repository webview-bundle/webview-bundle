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

export class BuildAppleCommand extends Command {
  static paths = [['build', 'apple']];

  readonly profile = ProfileOption;
  readonly targets = AppleTargetOption;

  async execute() {
    const targets = AppleTargetSchema.array()
      .nonempty()
      .parse(this.targets ?? AppleTargetSchema.options);

    const genDir = path.join(PKG_DIR, '.gen', 'apple');

    await fs.rm(genDir, { recursive: true, force: true });
    await fs.mkdir(genDir, { recursive: true });

    const headersDir = path.join(genDir, 'headers');
    const swiftDir = path.join(genDir, 'swift');
    const zipPath = path.join(PKG_DIR, '.gen', 'apple.zip');

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
        const libPath = await this.lipo(platform, platformLibPaths, genDir);
        libPaths.push(libPath);
      }
    }

    console.log('built libs:');
    for (const libPath of libPaths) {
      console.log(`- ${path.relative(ROOT_DIR, libPath)}`);
    }

    await generateUniffiBindings('swift', libPaths[0]!, genDir);
    await this.moveFiles('*.h', genDir, headersDir);
    await this.consolidateModulemapFiles(genDir, headersDir);
    await this.moveFiles('*.swift', genDir, swiftDir);

    const xcframeworkPath = path.join(genDir, `WebViewBundle.xcframework`);
    await this.createXCFramework(libPaths, headersDir, xcframeworkPath);
    console.log(`xcframework: ${path.relative(ROOT_DIR, xcframeworkPath)}`);

    await fs.rm(headersDir, { recursive: true });
    await this.zip(genDir, zipPath);
  }

  private async build(target: AppleTarget, profile: Profile) {
    const args = ['build', '-p', PKG_NAME, '--target', target, '--profile', profile];
    await runCommand('cargo', args, {
      cwd: ROOT_DIR,
      prefix: `[${target}]`,
    });

    const libPath = path.join(getProfileTargetDir(profile, target), `lib${LIB_NAME}.a`);
    return libPath;
  }

  private async lipo(platform: ApplePlatform, libPaths: string[], genDir: string) {
    const outputDir = path.join(genDir, 'lipo', platform);
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

  private async zip(genDir: string, zipPath: string) {
    await fs.rm(zipPath, { force: true });
    await fs.mkdir(path.dirname(zipPath), { recursive: true });

    const cwd = path.join(genDir, '..');

    const zipFile = path.relative(cwd, zipPath);
    const zipDir = path.relative(cwd, genDir);

    await runCommand('zip', ['-r', zipFile, zipDir], {
      cwd,
      prefix: '[zip] ',
    });
  }
}
