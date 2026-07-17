import fs from 'node:fs/promises';
import path from 'node:path';
import * as TOML from '@ltd/j-toml';
import taploLib from '@taplo/lib';
import { camelCase, escapeRegExp, mapKeys, mapValues } from 'es-toolkit';
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

/** The tables a dependency's version is propagated into, paired with their parsed entries. */
function dependencyTables(toml: CargoToml): Array<{ name: string; entries?: CargoDependencies }> {
  return [
    { name: 'dependencies', entries: toml.dependencies },
    { name: 'dev-dependencies', entries: toml['dev-dependencies'] },
    { name: 'workspace.dependencies', entries: toml.workspace?.dependencies },
  ];
}

/** The version a declared dependency pins, or `undefined` for e.g. `{ workspace = true }`. */
function pinnedVersionOf(value: CargoDependency | undefined): string | undefined {
  if (value == null) {
    return undefined;
  }
  return typeof value === 'string' ? value : value.version;
}

/** Every version the document declares for `dep` — or for `[package]`, when `dep` is absent. */
function versionsOf(toml: CargoToml, dep?: string): string[] {
  if (dep == null) {
    const version = toml.package?.version;
    return version != null ? [version] : [];
  }
  return dependencyTables(toml)
    .map(table => pinnedVersionOf(table.entries?.[dep]))
    .filter(declared => declared != null);
}

/**
 * Rewrite the entry `key` of table `table` through `edit`, in place. Only the one matching line is
 * handed to `edit`; every other byte of the document is left alone. Returns whether it was found
 * *and* rewritten (`edit` returns `null` when the line holds nothing it can set).
 */
function editEntry(
  lines: string[],
  table: string,
  key: string,
  edit: (line: string) => string | null
): boolean {
  // Matches `[table]` and `[[table]]` alike, so an array-of-tables also ends the preceding table.
  const header = /^\s*\[\[?([^[\]]*)\]\]?\s*$/;
  const entry = new RegExp(`^\\s*${escapeRegExp(key)}\\s*=`);
  let current: string | null = null;
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i]!;
    const matched = header.exec(line);
    if (matched != null) {
      current = matched[1]!.trim();
      continue;
    }
    if (current !== table || !entry.test(line)) {
      continue;
    }
    const edited = edit(line);
    if (edited == null) {
      return false;
    }
    lines[i] = edited;
    return true;
  }
  return false;
}

/** Replace the value of a `key = "<value>"` line, keeping its spacing and any trailing comment. */
function replaceStringValue(line: string, value: string): string | null {
  const matched = /^(\s*[^=]+=\s*)"[^"]*"(.*)$/.exec(line);
  return matched == null ? null : `${matched[1]}"${value}"${matched[2]}`;
}

/** Replace a dependency's version, in either the `dep = "1"` or `dep = { version = "1", .. }` form. */
function replaceDependencyVersion(line: string, value: string): string | null {
  const inline = /^(\s*[^=]+=\s*\{[^}]*?\bversion\s*=\s*)"[^"]*"(.*)$/.exec(line);
  if (inline != null) {
    return `${inline[1]}"${value}"${inline[2]}`;
  }
  return replaceStringValue(line, value);
}

/**
 * Set the `[package]` version of `raw` — or, with `dep`, the version every dependency table pins
 * `dep` at — and return the updated document.
 *
 * The edit is made on the text, and everything else is handed back byte for byte. Re-emitting a
 * parsed document instead would silently drop every comment, since the TOML parser does not retain
 * them: that quietly deleted the reasons behind `strip = "none"` and the `alloc-no-stdlib` pins on
 * the first release after they were written.
 */
export function editCargoTomlVersion(raw: string, version: Version, dep?: string): string {
  // A prerelease is pinned exactly, so a dependent never resolves across channels.
  const wanted =
    dep != null && version.prerelease != null ? `=${version.toString()}` : version.toString();
  const lines = raw.split('\n');

  if (dep == null) {
    if (!editEntry(lines, 'package', 'version', line => replaceStringValue(line, wanted))) {
      throw new Error('cannot find a `version` entry under `[package]`');
    }
  } else {
    for (const table of dependencyTables(parseCargoToml(raw))) {
      // Absent, or carrying no version of its own (`{ workspace = true }`) — nothing to set.
      if (pinnedVersionOf(table.entries?.[dep]) == null) {
        continue;
      }
      if (!editEntry(lines, table.name, dep, line => replaceDependencyVersion(line, wanted))) {
        throw new Error(`cannot set the version of \`${dep}\` under \`[${table.name}]\``);
      }
    }
  }

  // The edits are textual, so read them back through the parser to be sure each one landed on the
  // value it was aimed at, rather than on some other string sharing the line.
  const edited = lines.join('\n');
  const wrong = versionsOf(parseCargoToml(edited), dep).filter(declared => declared !== wanted);
  if (wrong.length > 0) {
    throw new Error(
      `failed to set \`${dep ?? '[package]'}\` to "${wanted}": got "${wrong.join('", "')}"`
    );
  }
  return edited;
}

export function formatCargoToml(raw: string): string {
  return taplo.format(raw, { options: taploConfig.formatting });
}
