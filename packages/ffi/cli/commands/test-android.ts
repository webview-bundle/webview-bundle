import path from 'node:path';
import { Command } from 'clipanion';
import { PKG_DIR } from '../consts.ts';
import { runCommand } from '../run.ts';

export class TestAndroidCommand extends Command {
  static paths = [['test', 'android']];

  async execute() {
    const androidDir = path.join(PKG_DIR, 'android');

    await runCommand('./gradlew', [':lib-jvm:test'], {
      cwd: androidDir,
      prefix: '[:lib-jvm:test] ',
    });
    await runCommand('./gradlew', [':testapp:assembleDebug'], {
      cwd: androidDir,
      prefix: '[:testapp:assembleDebug] ',
    });
  }
}
