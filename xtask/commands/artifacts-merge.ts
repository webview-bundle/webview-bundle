import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import glob from 'fast-glob';
import { loadConfig } from '../config.ts';
import { ColorModeOption, colors, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';

export class ArtifactsMergeCommand extends Command {
  static paths = [['artifacts', 'merge']];

  readonly configFilepath = Option.String('--config', 'xtask.json');
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    try {
      const config = await loadConfig(this.configFilepath);
      if (config.artifacts == null) {
        console.log(colors.warn('no artifacts config found. skip.'));
        return 0;
      }
      const allFiles = await Promise.all(
        config.artifacts.files.map(file =>
          glob(file.source, {
            cwd: ROOT_DIR,
            onlyFiles: true,
          })
        )
      );
      const files = allFiles.flat();
      if (files.length === 0) {
        console.log(colors.warn('no files found. skip.'));
        return 0;
      }
      console.log(`found ${colors.info(files.length)} file(s) to merge`);
      for (let i = 0; i < files.length; i += 1) {
        const progress = `[${i + 1}/${files.length}]`;
        const file = files[i]!;
        const src = path.join(ROOT_DIR, file);
        const dest = path.join(ROOT_DIR, config.artifacts.dir, file);
        await fs.mkdir(path.dirname(dest), { recursive: true });
        await fs.copyFile(src, dest);
        console.log(`${colors.success(progress)} ${file}: file copied`);
      }
      return 0;
    } catch (e) {
      console.error(e);
      return 1;
    }
  }
}
