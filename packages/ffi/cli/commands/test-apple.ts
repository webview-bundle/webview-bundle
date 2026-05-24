import path from 'node:path';
import { Command } from 'clipanion';
import { PKG_DIR } from '../consts.ts';
import { runCommand } from '../run.ts';

export class TestAppleCommand extends Command {
  static paths = [['test', 'apple']];

  async execute() {
    const appleDir = path.join(PKG_DIR, 'apple');
    const iosDir = path.join(appleDir, 'ios');

    await runCommand('swift', ['build'], {
      cwd: appleDir,
      prefix: '[swift] ',
    });
    await runCommand('swift', ['test'], {
      cwd: appleDir,
      prefix: '[swift] ',
    });
    await runCommand('tuist', ['generate', '--no-open'], {
      cwd: iosDir,
      prefix: '[tuist] ',
    });
    await runCommand(
      'xcodebuild',
      [
        'build',
        '-scheme',
        'TestApp',
        '-destination',
        'generic/platform=iOS Simulator',
        'CODE_SIGNING_ALLOWED=NO',
      ],
      {
        cwd: iosDir,
        prefix: '[xcodebuild] ',
      }
    );
  }
}
