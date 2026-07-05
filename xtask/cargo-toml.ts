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

type CargoDependency = string | { version?: string; package?: string };
type CargoDependencies = Record<string, CargoDependency>;

export interface CargoToml {
  package?: {
    name?: string;
    version?: string;
    publish?: boolean;
  };
  dependencies?: CargoDependencies;
  'dev-dependencies'?: CargoDependencies;
  'build-dependencies'?: CargoDependencies;
  workspace?: {
    dependencies?: CargoDependencies;
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
  const formatted = taplo.format(content.join('\n'), { options: taploConfig.formatting });
  return preferDoubleQuotes(formatted);
}

/**
 * j-toml renders every string as a single-quoted TOML *literal* string, but this repo's
 * Cargo.toml style (double-quoted *basic* strings) is what `taplo` and the source files use.
 * A blanket `'` → `"` replacement is unsafe: a literal string whose content contains a `"` —
 * e.g. the `[target.'cfg(target_os = "android")'.dependencies]` key — becomes invalid TOML
 * (`unclosed table, expected ]`) once its delimiters flip to double quotes.
 *
 * This walks the document token by token: basic strings are copied verbatim (so apostrophes
 * inside them are never touched), and a literal string is converted to a basic string only when
 * its content has no `"` or `\` that would require escaping. Otherwise it is left as a literal.
 */
function preferDoubleQuotes(toml: string): string {
  let out = '';
  for (let i = 0; i < toml.length; ) {
    const ch = toml[i];
    if (ch === '"') {
      // Basic string: copy through the matching close quote, honoring backslash escapes.
      out += ch;
      i++;
      while (i < toml.length) {
        const c = toml[i];
        out += c;
        i++;
        if (c === '\\' && i < toml.length) {
          out += toml[i];
          i++;
        } else if (c === '"') {
          break;
        }
      }
    } else if (ch === "'") {
      // Literal string: content runs to the next single quote on the same line.
      const end = toml.indexOf("'", i + 1);
      const newline = toml.indexOf('\n', i + 1);
      if (end !== -1 && (newline === -1 || end < newline)) {
        const inner = toml.slice(i + 1, end);
        out += inner.includes('"') || inner.includes('\\') ? `'${inner}'` : `"${inner}"`;
        i = end + 1;
      } else {
        out += ch;
        i++;
      }
    } else {
      out += ch;
      i++;
    }
  }
  return out;
}
