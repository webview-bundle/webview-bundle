import path from 'node:path';
import { Command, Option, UsageError } from 'clipanion';
import { isBoolean, isEnum } from 'typanion';
// biome-ignore lint/correctness/useImportExtensions: import json file
import pkg from '../package.json' with { type: 'json' };
import { initRepository } from './git.js';
import {
  detectPackageManager,
  install,
  PACKAGE_MANAGERS,
  type PackageManager,
  runPrefix,
  runScript,
  supportsOffline,
} from './pm.js';
import { inspectTarget, toBundleName, toProjectName, validateProjectName } from './project.js';
import {
  CancelledError,
  isCancel,
  isInteractive,
  promptConfirm,
  promptSelect,
  promptText,
} from './prompts.js';
import { materialize, planFiles, type RenderContext, substitute } from './render.js';
import {
  collectCaveats,
  collectPackages,
  loadManifest,
  type Template,
  type TemplateManifest,
  type TemplateStatus,
  templatesDir,
  unreleasedPackages,
} from './templates.js';
import * as ui from './ui.js';
import { resolveVersions, type VersionMap } from './versions.js';

interface Gating {
  readonly versions: VersionMap;
  readonly template: Map<string, readonly string[]>;
}

const STATUS_LABEL: Record<TemplateStatus, string> = {
  stable: '',
  caveat: 'caveat',
  experimental: 'experimental',
};

function decorate(name: string, status: TemplateStatus, gated: readonly string[]): string {
  if (gated.length > 0) {
    return `${name} ${ui.c.warn(`(unreleased: ${gated.join(', ')})`)}`;
  }
  const label = STATUS_LABEL[status];
  return label === '' ? name : `${name} ${ui.c.warn(`(${label})`)}`;
}

export class CreateCommand extends Command {
  static paths = [Command.Default];
  static usage = Command.Usage({
    description: 'Scaffold a Webview Bundle app.',
    details: `
      Creates a new project that packs a web app into a \`.wvb\` bundle and serves it from a native
      webview host. Runs interactively when arguments are omitted.
    `,
    examples: [
      ['Scaffold interactively', '$0'],
      [
        'Scaffold an Electron app packaged with electron-builder',
        '$0 my-app --template electron-builder',
      ],
      ['Accept every default without prompting', '$0 my-app --yes'],
      ['Preview the files without writing them', '$0 my-app --template electron-forge --dry-run'],
    ],
  });

  readonly directory = Option.String({ name: 'DIRECTORY', required: false });

  readonly template = Option.String('--template,-t', {
    description: 'Template to scaffold. Prompts when omitted.',
  });
  readonly pm = Option.String('--pm', {
    validator: isEnum(PACKAGE_MANAGERS),
    description: 'Package manager to use. [Default: auto-detected]',
  });
  readonly yes = Option.Boolean('--yes,-y', false, {
    description: 'Accept defaults and never prompt.',
  });
  readonly git = Option.String('--git', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Initialize a git repository. [Default: true]',
  });
  readonly installDeps = Option.String('--install', {
    tolerateBoolean: true,
    validator: isBoolean(),
    description: 'Install dependencies after scaffolding. [Default: true]',
  });
  readonly offline = Option.Boolean('--offline', false, {
    description: "Prefer the package manager's offline cache.",
  });
  readonly dryRun = Option.Boolean('--dry-run', false, {
    description: 'Print the files that would be written and change nothing.',
  });
  readonly overwrite = Option.Boolean('--overwrite', false, {
    description: 'Scaffold into a non-empty directory.',
  });
  readonly versionsFile = Option.String('--versions', {
    description: 'JSON file overriding the @wvb/* versions written into the generated app.',
  });
  readonly color = Option.String('--color', 'auto', {
    validator: isEnum(['off', 'on', 'auto'] as const),
    description: 'Color mode. [Default: "auto"]',
  });

  async execute(): Promise<number> {
    ui.configureColor(this.color);
    try {
      return await this.run();
    } catch (error) {
      if (isCancel(error)) {
        ui.blank();
        ui.warn('Cancelled.');
        ui.blank();
        return 130;
      }
      if (error instanceof UsageError) {
        throw error;
      }
      ui.blank();
      ui.error((error as Error).message);
      ui.blank();
      return 1;
    }
  }

  private get interactive(): boolean {
    return isInteractive() && !this.yes;
  }

  private async run(): Promise<number> {
    const started = Date.now();
    ui.intro(`v${pkg.version}`);

    const dir = templatesDir();
    const manifest = await loadManifest(dir);

    const directory = await this.resolveDirectory();
    const target = path.resolve(process.cwd(), directory);
    const projectName = toProjectName(target);
    const nameProblem = validateProjectName(projectName);
    if (nameProblem != null) {
      throw new Error(`"${projectName}" is not a usable project name. ${nameProblem}`);
    }

    await this.ensureTargetIsUsable(target, directory);

    const gating = await this.resolveGating(dir, manifest);
    const versions = gating.versions;

    const [, template] = await this.resolveTemplate(manifest, gating);

    const pm = this.pm ?? detectPackageManager();
    if (this.offline && !supportsOffline(pm)) {
      ui.warn(`${pm} has no offline flag; installing normally.`);
    }

    const ctx: RenderContext = {
      projectName,
      bundleName: toBundleName(projectName),
      pm,
      pmRun: runPrefix(pm),
      versions,
    };

    const caveats = collectCaveats(template);
    if (caveats.length > 0) {
      ui.callout(
        template.status === 'experimental'
          ? 'Experimental — read before you build'
          : 'Before you build',
        caveats,
        'warn'
      );
      if (this.interactive && !(await promptConfirm('Continue?', true))) {
        throw new CancelledError();
      }
    }

    const writes = await planFiles(dir, template.layers, ctx);

    ui.heading(
      this.dryRun ? `Would create ${ui.c.accent(directory)}` : `Creating ${ui.c.accent(directory)}`
    );
    for (const write of writes.slice(0, 12)) {
      ui.added(write.path);
    }
    if (writes.length > 12) {
      ui.added(ui.c.muted(`…and ${writes.length - 12} more`));
    }

    if (this.dryRun) {
      ui.blank();
      ui.info(`${writes.length} files. Nothing was written (--dry-run).`);
      ui.blank();
      return 0;
    }

    await materialize(writes, target);
    ui.blank();
    ui.step(`${writes.length} files written`);

    const hasNodeManifest = writes.some(write => write.path === 'package.json');
    if ((this.installDeps ?? true) && hasNodeManifest) {
      await this.installDependencies(pm, target);
    }

    if (this.git ?? true) {
      const created = await initRepository(target);
      if (created) {
        ui.step('Git repository initialized');
      }
    }

    this.printNextSteps(template, directory, ctx, started);
    return 0;
  }

  private async installDependencies(pm: PackageManager, target: string): Promise<void> {
    ui.blank();
    ui.info(`Installing dependencies with ${pm}…`);
    try {
      await install(pm, target, { offline: this.offline });
      ui.step('Dependencies installed');
    } catch (error) {
      ui.warn(`Install failed: ${(error as Error).message}`);
      ui.info('Scaffolding is complete — install by hand to continue.');
    }
  }

  private async resolveDirectory(): Promise<string> {
    if (this.directory != null) {
      return this.directory;
    }
    if (!this.interactive) {
      if (this.yes) {
        return 'my-wvb-app';
      }
      throw new Error(
        'DIRECTORY is required when running non-interactively. Pass a directory or --yes.'
      );
    }
    return promptText('Project directory', {
      default: 'my-wvb-app',
      validate: value =>
        value === '' ? 'Project directory is required.' : validateProjectName(toProjectName(value)),
    });
  }

  private async ensureTargetIsUsable(target: string, directory: string): Promise<void> {
    const state = await inspectTarget(target);
    if (!state.exists || state.conflicts.length === 0 || this.overwrite) {
      return;
    }
    const preview = state.conflicts.slice(0, 4).join(', ');
    const more = state.conflicts.length > 4 ? `, …${state.conflicts.length - 4} more` : '';
    ui.warn(`${ui.c.accent(directory)} is not empty (${preview}${more}).`);
    if (!this.interactive) {
      throw new Error(`"${directory}" is not empty. Pass --overwrite to scaffold into it anyway.`);
    }
    if (
      !(await promptConfirm('Scaffold into it anyway? Existing files may be overwritten.', false))
    ) {
      throw new CancelledError();
    }
  }

  /**
   * Resolves the latest published version of every package the templates reference, then records
   * which templates are blocked because a package has not been released yet.
   */
  private async resolveGating(dir: string, manifest: TemplateManifest): Promise<Gating> {
    const template = new Map<string, readonly string[]>();
    const union = new Set<string>();

    for (const [id, spec] of Object.entries(manifest)) {
      const base = await collectPackages(dir, spec.layers);
      template.set(id, base);
      for (const pkg of base) {
        union.add(pkg);
      }
    }

    if (union.size > 0 && this.versionsFile == null && process.env.WVB_TEMPLATE_VERSIONS == null) {
      ui.info('Resolving the latest versions…');
    }
    const versions = await resolveVersions([...union], this.versionsFile);
    return { versions, template };
  }

  private async resolveTemplate(
    manifest: TemplateManifest,
    gating: Gating
  ): Promise<[string, Template]> {
    const ids = Object.keys(manifest);
    const gatedFor = (id: string) =>
      unreleasedPackages(gating.template.get(id) ?? [], gating.versions);

    if (this.template != null) {
      const template = manifest[this.template];
      if (template == null) {
        throw new Error(`Unknown template "${this.template}". Available: ${ids.join(', ')}`);
      }
      const gated = gatedFor(this.template);
      if (gated.length > 0) {
        throw new Error(
          `Template "${this.template}" needs ${gated.join(', ')}, which ${gated.length > 1 ? 'have' : 'has'} not been published yet.`
        );
      }
      return [this.template, template];
    }

    if (!this.interactive) {
      const fallback = ids.find(id => gatedFor(id).length === 0);
      if (this.yes && fallback != null) {
        return [fallback, manifest[fallback] as Template];
      }
      throw new Error(
        `--template is required when running non-interactively. Available: ${ids.join(', ')}`
      );
    }

    const id = await promptSelect(
      'Template',
      ids.map(key => {
        const template = manifest[key] as Template;
        const gated = gatedFor(key);
        return {
          name: decorate(template.name, template.status, gated),
          value: key,
          description: `  ${template.description}`,
          disabled: gated.length > 0 ? `(needs a published ${gated.join(', ')})` : false,
        };
      })
    );
    return [id, manifest[id] as Template];
  }

  private printNextSteps(
    template: Template,
    directory: string,
    ctx: RenderContext,
    started: number
  ): void {
    ui.rule();
    ui.outro(ctx.projectName, template.name, Date.now() - started);
    ui.heading('Next steps');
    ui.command(`cd ${directory}`);
    for (const line of template.nextSteps ?? [`${runScript(ctx.pm as PackageManager, 'dev')}`]) {
      ui.command(substitute(line, ctx, 'templates.json'));
    }
    ui.blank();
    ui.link('Docs', 'https://wvb.dev');
    ui.blank();
  }
}
