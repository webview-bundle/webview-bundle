import path from 'node:path';
import { loadEnvFile } from 'node:process';
import { camelCase } from 'es-toolkit';
import z from 'zod';

const EnvSchema = z.object({
  remote: z.object({
    endpoint: z.string(),
  }),
  cloudflare: z.object({
    accountId: z.string(),
    bucket: z.string(),
    kvNamespaceId: z.string(),
    accessKeyId: z.string(),
    secretAccessKey: z.string(),
    apiToken: z.string(),
  }),
});
export type Env = z.infer<typeof EnvSchema>;

export function loadEnv(configFile?: string): Env {
  // Env file should be placed at 'playground/.env`
  loadEnvFile(configFile ?? path.join(import.meta.dirname, '..', '..', '..', '.env'));

  const raw: any = {};

  for (const [key, value] of Object.entries(process.env)) {
    if (!key.startsWith('WVB_PLAYGROUND_')) {
      continue;
    }

    const [categoryRaw, ...nameRaw] = key.replace(/^WVB_PLAYGROUND_/, '').split('_');
    if (categoryRaw == null || nameRaw.length === 0) {
      continue;
    }
    const category = camelCase(categoryRaw);
    const name = camelCase(nameRaw.join('_'));

    raw[category] ??= {};
    raw[category][name] = value;
  }

  return EnvSchema.parse(raw);
}
