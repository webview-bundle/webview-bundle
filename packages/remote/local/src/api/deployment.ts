import fs from 'node:fs/promises';
import path from 'node:path';
import { glob } from 'tinyglobby';
import { z } from 'zod';
import { normalizeBundleName } from '../utils.js';

export const DeploymentFileSchema = z.object({
  name: z.string().describe('The name of the bundle'),
  version: z.string().describe('Current deployed version of the bundle').optional(),
  channels: z
    .record(z.string(), z.string())
    .optional()
    .describe('Version deployed in each channel'),
});
export type DeploymentFile = z.infer<typeof DeploymentFileSchema>;

interface ReadDeploymentParams {
  bundle: string;
  baseDir: string;
}

export async function readDeployment({
  bundle,
  baseDir,
}: ReadDeploymentParams): Promise<DeploymentFile | null> {
  const filePath = getDeploymentFilePath(baseDir, bundle);
  try {
    const raw = await fs.readFile(filePath, 'utf8');
    const parsed = DeploymentFileSchema.parse(JSON.parse(raw));
    return parsed;
  } catch {
    return null;
  }
}

interface ReadAllDeploymentsParams {
  baseDir: string;
}

export async function readAllDeployments({
  baseDir,
}: ReadAllDeploymentsParams): Promise<DeploymentFile[]> {
  const filePaths = await glob('*/deployment.json', {
    cwd: path.join(baseDir, 'bundles'),
    absolute: true,
    onlyFiles: true,
  });
  const deployments: DeploymentFile[] = [];
  for (const file of filePaths) {
    try {
      const raw = await fs.readFile(file, 'utf8');
      const parsed = DeploymentFileSchema.parse(JSON.parse(raw));
      deployments.push(parsed);
    } catch {
      //
    }
  }
  return deployments;
}

interface WriteDeploymentParams {
  baseDir: string;
  bundle: string;
  version: string;
  channel?: string;
}

export async function writeDeployment({
  baseDir,
  bundle,
  version,
  channel,
}: WriteDeploymentParams): Promise<void> {
  const deployment = (await readDeployment({ baseDir, bundle })) ?? {
    name: bundle,
    version: undefined,
    channels: {},
  };

  if (channel != null) {
    deployment.channels ??= {};
    deployment.channels[channel] = version;
  } else {
    deployment.version = version;
  }

  const filePath = getDeploymentFilePath(baseDir, bundle);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, JSON.stringify(deployment, null, 2), 'utf8');
}

function getDeploymentFilePath(baseDir: string, bundle: string): string {
  return path.join(baseDir, 'bundles', `${normalizeBundleName(bundle)}`, 'deployment.json');
}
