import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import glob from 'fast-glob';
import { minimatch } from 'minimatch';
import { loadConfig } from '../config.ts';
import { ColorModeOption, colors, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';

export class ArtifactsSpreadCommand extends Command {
  static paths = [['artifacts', 'spread']];

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
      const files = await glob('**/*', {
        cwd: path.join(ROOT_DIR, config.artifacts.dir),
        onlyFiles: true,
      });
      if (files.length === 0) {
        console.log(colors.warn('no files found. skip.'));
        return 0;
      }
      console.log(`found ${colors.info(files.length)} file(s) to spread`);
      for (let i = 0; i < files.length; i += 1) {
        const progress = `[${i + 1}/${files.length}]`;
        const file = files[i]!;
        const artifactFile = config.artifacts.files.find(x => minimatch(file, x.source));
        if (artifactFile == null) {
          console.log(`${colors.warn(progress)} ${file}: skip because no artifact file found`);
          continue;
        }
        const src = path.join(ROOT_DIR, config.artifacts.dir, file);
        const dest = path.join(ROOT_DIR, artifactFile.dist, path.basename(file));
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
