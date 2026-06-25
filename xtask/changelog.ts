import fs from 'node:fs/promises';
import path from 'node:path';
import type { Action } from './action.ts';
import type { Changes } from './changes.ts';
import { ROOT_DIR } from './consts.ts';
import type { Package } from './package.ts';

export class Changelog {
  public readonly path: string;
  private readonly raw: string;
  private readonly lines: string[];

  static async load(filepath: string): Promise<Changelog> {
    const absolutePath = path.isAbsolute(filepath) ? filepath : path.join(ROOT_DIR, filepath);
    const content = await fs.readFile(absolutePath, 'utf8');
    return new Changelog(filepath, content);
  }

  constructor(path: string, raw: string) {
    this.path = path;
    this.raw = raw;
    this.lines = raw.split('\n');
  }

  get hasChanged(): boolean {
    return this.raw !== this.lines.join('\n');
  }

  appendChanges(pkg: Package, changes: Changes): void {
    if (!pkg.hasChanged) {
      return;
    }
    const title = this.formatTitle(pkg);
    const changesLines = changes.changes.map(x => `- ${x.toString()}`);
    const idx = this.lines.findIndex(x => x.startsWith(title));
    if (idx > -1) {
      this.lines.splice(idx + 2, 0, ...changesLines);
    } else {
      const packages = pkg.versionedFiles
        .filter(x => x.canPublish)
        // biome-ignore lint/suspicious/useIterableCallbackReturn: ts issue
        .map((file): string => {
          const name = file.name;
          const version = file.nextVersion.toString();
          switch (file.type) {
            case 'package.json':
              return `[\`${name}\`](https://www.npmjs.com/package/${name}/v/${version})`;
            case 'Cargo.toml':
              return `[\`${name}\`](https://crates.io/crates/${name}/${version})`;
            case 'deno.json':
              return `[\`${name}\`](https://jsr.io/${name}@${version})`;
          }
        })
        .join(', ');
      const lines = [
        '',
        title,
        '',
        `This release includes packages: ${packages}`,
        '',
        ...changesLines,
      ];
      if (this.lines.length > 1) {
        this.lines.splice(1, 0, ...lines);
      } else {
        this.lines.push(...lines);
      }
    }
  }

  extractChanges(pkg: Package): string | null {
    const lines = [];
    const title = this.formatTitle(pkg);
    let idx = this.lines.findIndex(x => x.startsWith(title));
    if (idx === -1) {
      return null;
    }
    idx += 1;
    while (this.lines[idx] != null) {
      if (this.lines[idx]?.startsWith('## ') === true) {
        break;
      }
      lines.push(this.lines[idx]!);
      idx += 1;
    }
    return this.lines.join('\n');
  }

  write(): Action[] {
    if (!this.hasChanged) {
      return [];
    }
    return [
      {
        type: 'write',
        path: this.path,
        content: this.lines.join('\n'),
        prevContent: this.raw,
      },
    ];
  }

  private formatTitle(pkg: Package): string {
    return `## ${pkg.name} v${pkg.nextVersion.toString()}`;
  }
}
