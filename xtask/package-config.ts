import fs from 'node:fs/promises';
import { z } from 'zod';

const ArtifactSchema = z.object({
  src: z.string().describe('Source directory for the artifact files'),
  patterns: z.array(z.string()).describe('Artifact file patterns to include'),
  dest: z.string().describe('Destination directory for the artifact files'),
});
export type Artifact = z.infer<typeof ArtifactSchema>;

const ScriptSchema = z.object({
  command: z.string().describe('Command to run'),
  args: z.array(z.string()).optional().describe('Command arguments'),
  cwd: z.string().optional().describe('Working directory for the command'),
});
export type Script = z.infer<typeof ScriptSchema>;

export const PackageConfigSchema = z.object({
  name: z.string().optional().describe('Package name'),
  changelog: z.string().optional().describe('Changelog file path [Default: CHANGELOG.md]'),
  scopes: z.array(z.string()).optional().describe('Additional package scopes'),
  artifacts: z.array(ArtifactSchema).optional().describe('Package artifacts config'),
  beforePublishScripts: z
    .array(ScriptSchema)
    .optional()
    .describe('Scripts to run before publishing'),
  assets: z.array(z.string()).optional().describe('Assets to include in the package'),
});
export type PackageConfig = z.infer<typeof PackageConfigSchema>;

export async function loadPackageConfig(configFilePath: string): Promise<PackageConfig> {
  const raw = await fs.readFile(configFilePath, 'utf8');
  try {
    const json = JSON.parse(raw);

    return PackageConfigSchema.parse(json);
  } catch (e: any) {
    console.error(`Fail to load package config: ${e.message}`);
    throw e;
  }
}
