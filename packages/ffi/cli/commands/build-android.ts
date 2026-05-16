import fs from 'node:fs/promises';
import path from 'node:path';
import { Command } from 'clipanion';
import { LIB_NAME, PKG_NAME } from '../cargo.ts';
import { PKG_DIR, ROOT_DIR } from '../consts.ts';
import { getProfileTargetDir, type Profile, ProfileOption } from '../profile.ts';
import { runCommand } from '../run.ts';
import { type AndroidTarget, AndroidTargetOption, AndroidTargetSchema } from '../target.ts';
import { generateUniffiBindings } from '../uniffi.ts';

export class BuildAndroidCommand extends Command {
  static paths = [['build', 'android']];

  readonly profile = ProfileOption;
  readonly targets = AndroidTargetOption;

  async execute() {
    const targets = AndroidTargetSchema.array()
      .nonempty()
      .parse(this.targets ?? AndroidTargetSchema.options);

    const genDir = path.join(PKG_DIR, '.gen', 'android');

    await fs.rm(genDir, { recursive: true, force: true });
    await fs.mkdir(genDir, { recursive: true });

    const jniDir = path.join(genDir, 'jniLibs');
    const kotlinDir = path.join(genDir, 'kotlin');
    const zipPath = path.join(PKG_DIR, '.gen', 'android.zip');

    let libPath: string;

    for (const target of targets) {
      libPath = await this.build(target, this.profile, jniDir);
    }

    await generateUniffiBindings('kotlin', libPath!, kotlinDir);
    await this.zip(genDir, zipPath);
  }

  private async build(target: AndroidTarget, profile: Profile, jniDir: string) {
    const args = [
      'ndk',
      '--target',
      target,
      '-o',
      jniDir,
      'build',
      '--profile',
      profile,
      '--package',
      PKG_NAME,
    ];

    await runCommand('cargo', args, {
      cwd: ROOT_DIR,
      prefix: `[${target}]`,
    });

    const libPath = path.join(getProfileTargetDir(profile, target), `lib${LIB_NAME}.so`);
    return libPath;
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
