import fs from 'node:fs/promises';
import path from 'node:path';
import * as TOML from '@ltd/j-toml';
import { PKG_DIR } from './consts.ts';

const cargoRaw = await fs.readFile(path.join(PKG_DIR, 'Cargo.toml'), 'utf-8');
const toml: any = TOML.parse(cargoRaw);

export const PKG_NAME: string = toml.package.name;
export const LIB_NAME: string = toml.lib.name;
