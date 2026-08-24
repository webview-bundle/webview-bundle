import fs from 'node:fs/promises';
import path from 'node:path';
import { Command } from 'clipanion';
import { LIB_NAME, PKG_NAME } from '../cargo.ts';
import { PKG_DIR, ROOT_DIR } from '../consts.ts';
import { getProfileTargetDir, type Profile, ProfileOption } from '../profile.ts';
import { runCommand } from '../run.ts';
import { type AndroidTarget, AndroidTargetOption, AndroidTargetSchema } from '../target.ts';
import { generateUniffiBindings } from '../uniffi.ts';
import { zip } from '../zip.ts';

export class BuildAndroidCommand extends Command {
  static paths = [['build', 'android']];

  readonly profile = ProfileOption;
  readonly targets = AndroidTargetOption;

  async execute() {
    const targets = AndroidTargetSchema.array()
      .nonempty()
      .parse(this.targets ?? AndroidTargetSchema.options);

    const androidDir = path.join(PKG_DIR, 'android');

    const jniDir = path.join(androidDir, 'lib', 'src', 'main', 'jniLibs');
    const zipPath = path.join(PKG_DIR, '.output', 'android.zip');

    await fs.rm(jniDir, { force: true, recursive: true });

    let libPath: string;

    for (const target of targets) {
      libPath = await this.buildJniLibs(target, this.profile, jniDir);
    }

    const kotlinDir = path.join(androidDir, 'lib', 'src', 'main', 'kotlin');
    await fs.mkdir(kotlinDir, { recursive: true });
    await generateUniffiBindings('kotlin', libPath!, kotlinDir);
    await this.zip(androidDir, zipPath);
  }

  private async buildJniLibs(target: AndroidTarget, profile: Profile, jniDir: string) {
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
      prefix: `[buildJniLibs:${target}]`,
    });

    const libPath = path.join(getProfileTargetDir(profile, target), `lib${LIB_NAME}.so`);
    return libPath;
  }

  private async zip(androidDir: string, zipPath: string) {
    await fs.rm(zipPath, { force: true });
    await fs.mkdir(path.dirname(zipPath), { recursive: true });

    await zip(zipPath, androidDir, ['lib/src/main/jniLibs/**', 'lib/src/main/kotlin/**/*']);
  }
}
