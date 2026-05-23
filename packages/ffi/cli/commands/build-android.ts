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

    const genDir = path.join(PKG_DIR, 'gen', 'android');

    const jniDir = path.join(genDir, 'jniLibs');
    const jniDirForTests = path.join(genDir, 'jniLibsForTests');
    const kotlinDir = path.join(genDir, 'kotlin');
    const zipPath = path.join(PKG_DIR, 'gen', 'android.zip');

    await Promise.all(
      [jniDir, jniDirForTests, kotlinDir].map(dir => fs.rm(dir, { force: true, recursive: true }))
    );

    let libPath: string;

    for (const target of targets) {
      libPath = await this.build(target, this.profile, jniDir);
    }

    await generateUniffiBindings('kotlin', libPath!, kotlinDir);
    await this.moveTestJniLib(jniDirForTests);
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

  private async moveTestJniLib(destDir: string) {
    const fileName = `lib${LIB_NAME}.dylib`;
    const src = path.join(ROOT_DIR, 'target', 'release', fileName);
    const dest = path.join(destDir, fileName);

    await fs.mkdir(destDir, { recursive: true });
    await fs.copyFile(src, dest);
  }

  private async zip(genDir: string, zipPath: string) {
    await fs.rm(zipPath, { force: true });
    await fs.mkdir(path.dirname(zipPath), { recursive: true });

    await zip(zipPath, genDir, ['jniLibs/**', 'kotlin/**']);
  }
}
