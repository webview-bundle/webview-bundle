import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { toJSONSchema } from 'zod/v4';
import { PackageConfigSchema } from '../package-config.ts';

const dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.join(dirname, '..', '..');

const schema = toJSONSchema(PackageConfigSchema);
await fs.writeFile(path.join(rootDir, 'xtask.$schema.json'), JSON.stringify(schema, null, 2));
