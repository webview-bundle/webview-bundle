import fs from 'node:fs/promises';
import path from 'node:path';
import * as TOML from '@ltd/j-toml';
import taploLib from '@taplo/lib';
import { camelCase, mapKeys, mapValues } from 'es-toolkit';
import { ROOT_DIR } from './consts.ts';
import type { Version } from './version.ts';

const taplo = await taploLib.Taplo.initialize();
const taploConfigRaw = await fs.readFile(path.join(ROOT_DIR, 'taplo.toml'), 'utf8');

function deepKeyToCamelCase(x: any): any {
  if (Array.isArray(x)) {
    return x.map(deepKeyToCamelCase);
  }
  if (x != null && typeof x === 'object') {
    const withValues = mapValues(x, deepKeyToCamelCase);
    return mapKeys(withValues, (_v, key) => (typeof key === 'string' ? camelCase(key) : key));
  }
  return x;
}

const taploConfig: any = deepKeyToCamelCase(TOML.parse(taploConfigRaw, { bigint: false }));

export interface CargoToml {
  package?: {
    name?: string;
    version?: string;
    publish?: boolean;
  };
  dependencies?: Record<string, string | { version?: string }>;
  'dev-dependencies'?: Record<string, string | { version?: string }>;
  workspace?: {
    dependencies?: Record<string, string | { version?: string }>;
  };
}

export function parseCargoToml(raw: string): CargoToml {
  return TOML.parse(raw);
}

export function editCargoTomlVersion(toml: CargoToml, version: Version, dep?: string): void {
  const ver = version.prerelease != null ? `=${version.toString()}` : version.toString();
  if (dep != null) {
    if (toml.dependencies?.[dep] != null) {
      if (typeof toml.dependencies[dep] === 'string') {
        toml.dependencies[dep] = ver;
      } else if (typeof toml.dependencies[dep]?.version === 'string') {
        toml.dependencies[dep].version = ver;
      }
    }
    if (toml['dev-dependencies']?.[dep] != null) {
      if (typeof toml['dev-dependencies'][dep] === 'string') {
        toml['dev-dependencies'][dep] = ver;
      } else if (typeof toml['dev-dependencies'][dep]?.version === 'string') {
        toml['dev-dependencies'][dep].version = ver;
      }
    }
    if (toml.workspace?.dependencies?.[dep] != null) {
      if (typeof toml.workspace?.dependencies?.[dep] === 'string') {
        toml.workspace.dependencies[dep] = ver;
      } else if (typeof toml.workspace.dependencies[dep]?.version === 'string') {
        toml.workspace.dependencies[dep].version = ver;
      }
    }
  } else {
    toml.package ??= {};
    toml.package.version = version.toString();
  }
}

export function formatCargoToml(toml: CargoToml): string {
  let content = TOML.stringify(toml as any) as any as string[];
  content = content.slice(1);
  content = content.filter((line, i) => {
    const prevLine = content[i - 1];
    return !(prevLine?.startsWith('[') === true && line === '');
  });
  return taplo
    .format(content.join('\n'), { options: taploConfig.formatting })
    .replaceAll(/'/g, '"');
}
