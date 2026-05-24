import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import { glob } from 'tinyglobby';
import { ColorModeOption, colors, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';
import { Package } from '../package.ts';

/**
 * Merge artifacts from all packages into one directory.
 * This can be used to combine results generated from different architectures in CI environments.
 */
export class ArtifactsMergeCommand extends Command {
  static paths = [['artifacts', 'merge']];

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
          const srcDir = path.join(ROOT_DIR, pkg.path, artifact.src);
          const files = await glob(artifact.patterns, {
            cwd: srcDir,
            onlyFiles: true,
          });

          if (files.length === 0) {
            console.log(colors.warn(`[${pkg.name}] no artifacts files found. skip.`));
            continue;
          }

          console.log(`[${pkg.name}] found ${colors.info(files.length)} file(s) to merge`);
          for (let i = 0; i < files.length; i += 1) {
            const progress = `[${i + 1}/${files.length}]`;
            const file = files[i]!;
            const src = path.join(srcDir, file);
            const dest = path.join(mergeBaseDir, file);
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
