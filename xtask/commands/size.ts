import fs from 'node:fs/promises';
import path from 'node:path';
import { Command, Option } from 'clipanion';
import { glob } from 'tinyglobby';
import { ColorModeOption, colors, setColorMode } from '../console.ts';
import { ROOT_DIR } from '../consts.ts';
import { createGitHubClient } from '../github.ts';
import { Package } from '../package.ts';
import {
  type Baseline,
  collectSizes,
  compareSizes,
  fetchBaseline,
  formatBytes,
  mergeReports,
  parseReport,
  renderComparison,
  reportMarker,
  upsertPullRequestComment,
} from '../size.ts';

/** Measure the native binaries built in this checkout and write a size report (JSON). */
export class SizeCollectCommand extends Command {
  static paths = [['size', 'collect']];

  readonly out = Option.String('--out', 'size-report.json', {
    description: 'Path to write the size report to',
  });
  readonly mergeDir = Option.String('--merge-dir', {
    required: false,
    description: 'Read from an `artifacts merge` directory instead of the build outputs',
  });
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    const packages = await Package.loadAll();
    const report = await collectSizes(packages, this.mergeDir);
    for (const [pkg, sizes] of Object.entries(report.packages)) {
      for (const [name, size] of Object.entries(sizes)) {
        console.log(`${colors.info(`[${pkg}]`)} ${name}: ${formatBytes(size)}`);
      }
    }
    if (Object.keys(report.packages).length === 0) {
      console.log(colors.warn('no native binaries found.'));
    }
    const out = path.resolve(ROOT_DIR, this.out);
    await fs.mkdir(path.dirname(out), { recursive: true });
    await fs.writeFile(out, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
    console.log(`size report written: ${out}`);
    return 0;
  }
}

/**
 * Compare size reports against each package's latest stable GitHub release and publish the result
 * to the step summary and (optionally) a sticky PR comment.
 */
export class SizeCompareCommand extends Command {
  static paths = [['size', 'compare']];

  readonly current = Option.String('--current', 'size-report', {
    description: 'Size report file, or a directory of them to merge',
  });
  readonly id = Option.String('--id', 'native', {
    description: 'Report id; one PR comment is kept per id',
  });
  readonly title = Option.String('--title', 'Native binary size', {
    description: 'Report heading',
  });
  readonly threshold = Option.String('--threshold', '5', {
    description: 'Percent increase flagged with a warning',
  });
  readonly pr = Option.String('--pr', { required: false, description: 'PR number to comment on' });
  readonly commit = Option.String('--commit', { required: false });
  readonly out = Option.String('--out', { required: false, description: 'Write Markdown here' });
  readonly githubToken = Option.String('--github-token', { required: false, env: 'GITHUB_TOKEN' });
  readonly githubStepSummary = Option.String('--github-step-summary', {
    required: false,
    env: 'GITHUB_STEP_SUMMARY',
  });
  readonly colorMode = ColorModeOption;

  async execute() {
    setColorMode(this.colorMode);
    if (this.githubToken == null) {
      console.error(colors.error('--github-token (or GITHUB_TOKEN) is required'));
      return 1;
    }
    const current = mergeReports(await this.readReports());
    const client = createGitHubClient(this.githubToken);

    const baselines = new Map<string, Baseline | null>();
    for (const pkg of Object.keys(current.packages)) {
      const baseline = await fetchBaseline(client, pkg);
      console.log(
        `${colors.info(`[${pkg}]`)} baseline: ${baseline?.tag ?? colors.warn('no stable release')}`
      );
      baselines.set(pkg, baseline);
    }

    const body = renderComparison(compareSizes(current, baselines), {
      id: this.id,
      title: this.title,
      threshold: Number(this.threshold),
      commit: this.commit,
    });
    console.log(body);

    if (this.out != null) {
      await fs.writeFile(path.resolve(ROOT_DIR, this.out), body, 'utf8');
    }
    if (this.githubStepSummary != null) {
      await fs.appendFile(this.githubStepSummary, body, 'utf8');
    }
    if (this.pr != null) {
      try {
        const url = await upsertPullRequestComment(
          client,
          Number(this.pr),
          reportMarker(this.id),
          body
        );
        console.log(`PR comment: ${url}`);
      } catch (e) {
        // PRs from forks get a read-only token; the step summary still carries the report.
        console.log(colors.warn(`could not comment on PR #${this.pr}: ${(e as Error).message}`));
      }
    }
    return 0;
  }

  private async readReports() {
    const target = path.resolve(ROOT_DIR, this.current);
    const stat = await fs.stat(target);
    const files = stat.isDirectory()
      ? (await glob('**/*.json', { cwd: target, absolute: true })).sort()
      : [target];
    return Promise.all(files.map(async file => parseReport(await fs.readFile(file, 'utf8'))));
  }
}
