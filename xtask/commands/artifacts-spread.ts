import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import { glob } from 'tinyglobby';
import { ColorModeOption, colors, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';
import { Package } from '../package.ts';

export class ArtifactsSpreadCommand extends Command {
  static paths = [['artifacts', 'spread']];

  readonly mergeDir = Option.String('--merge-dir', 'artifacts-merged', {
    description: 'Directory to merge artifacts into',
  });
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    try {
      const packages = await Package.loadAll();

      for (const pkg of packages) {
        if (pkg.artifacts.length === 0) {
          continue;
        }

        const mergeBaseDir = path.join(ROOT_DIR, this.mergeDir, pkg.path);

        for (const artifact of pkg.artifacts) {
          const files = await glob('**/*', {
            cwd: mergeBaseDir,
            onlyFiles: true,
          });

          if (files.length === 0) {
            console.log(colors.warn(`[${pkg.name}] no files found. skip.`));
            continue;
          }

          console.log(`found ${colors.info(files.length)} file(s) to spread`);
          for (let i = 0; i < files.length; i += 1) {
            const progress = `[${i + 1}/${files.length}]`;
            const file = files[i]!;
            const src = path.join(mergeBaseDir, file);
            const dest = path.join(ROOT_DIR, pkg.path, artifact.dest, file);
            await fs.mkdir(path.dirname(dest), { recursive: true });
            await fs.copyFile(src, dest);
            console.log(`${colors.success(progress)} ${file}: file copied`);
          }
        }
      }

      return 0;
    } catch (e) {
      console.error(e);
      return 1;
    }
  }
}
