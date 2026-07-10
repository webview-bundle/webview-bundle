import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { z } from 'zod';
import { ROOT_DIR } from './consts.ts';

const ArtifactSchema = z.strictObject({
  src: z.string().describe('Source directory for the artifact files'),
  patterns: z.array(z.string()).describe('Artifact file patterns to include'),
  dest: z.string().describe('Destination directory for the artifact files'),
});
export type Artifact = z.infer<typeof ArtifactSchema>;

const ScriptSchema = z.strictObject({
  command: z.string().describe('Command to run'),
  args: z.array(z.string()).optional().describe('Command arguments'),
  cwd: z.string().optional().describe('Working directory for the command'),
});
export type Script = z.infer<typeof ScriptSchema>;

const PackageConfigSchema = z.strictObject({
  name: z.string().optional().describe('Package name [Default: directory name]'),
  changelog: z.string().optional().describe('Changelog file path [Default: CHANGELOG.md]'),
  artifacts: z.array(ArtifactSchema).optional().describe('Package artifacts config'),
  beforePublishScripts: z
    .array(ScriptSchema)
    .optional()
    .describe('Scripts to run before publishing'),
  assets: z.array(z.string()).optional().describe('Assets to include in the package'),
});
export type PackageConfig = z.infer<typeof PackageConfigSchema>;

const PackageEntrySchema = z.union([
  z.string().describe('Glob of package directories, released with the default config'),
  PackageConfigSchema.extend({
    path: z.string().describe('Package directory, relative to the repository root'),
  }),
]);
export type PackageEntry = z.infer<typeof PackageEntrySchema>;

const XtaskConfigSchema = z.strictObject({
  packages: z.array(PackageEntrySchema).describe('Packages to version and release'),
});
export type XtaskConfig = z.infer<typeof XtaskConfigSchema>;

/** Identity helper so `xtask.config.ts` gets type checking and editor completion. */
export function defineConfig(config: XtaskConfig): XtaskConfig {
  return config;
}

export const CONFIG_FILE = 'xtask.config.ts';

/**
 * Load the root `xtask.config.ts` with a native `import()` (Node strips the types) and validate
 * its default export. Validation is strict: unknown fields are an error, so stale config cannot
 * accumulate silently.
 */
export async function loadXtaskConfig(): Promise<XtaskConfig> {
  const filepath = path.join(ROOT_DIR, CONFIG_FILE);
  let mod: { default?: unknown };
  try {
    mod = await import(pathToFileURL(filepath).href);
  } catch (e: any) {
    throw new Error(`Fail to load "${CONFIG_FILE}": ${e.message}`, { cause: e });
  }
  const result = XtaskConfigSchema.safeParse(mod.default);
  if (!result.success) {
    throw new Error(`Invalid "${CONFIG_FILE}": ${z.prettifyError(result.error)}`);
  }
  return result.data;
}
