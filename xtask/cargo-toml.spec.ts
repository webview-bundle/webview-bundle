import * as TOML from '@ltd/j-toml';
import { describe, expect, it } from 'vitest';
import { editCargoTomlVersion, formatCargoToml, parseCargoToml } from './cargo-toml.ts';
import { Version } from './version.ts';

function bump(raw: string, version: string): string {
  return formatCargoToml(editCargoTomlVersion(raw, Version.parse(version)));
}

function bumpDep(raw: string, dep: string, version: string): string {
  return editCargoTomlVersion(raw, Version.parse(version), dep);
}

describe('editCargoTomlVersion', () => {
  it('bumps the package version', () => {
    const out = bump('[package]\nname = "demo"\nversion = "0.1.0"\n', '0.1.0-next.abc1234');
    expect(out).toContain('version = "0.1.0-next.abc1234"');
    expect(() => TOML.parse(out)).not.toThrow();
  });

  it('keeps comments, which the release commit used to delete', () => {
    // Regression: the version was set by re-emitting the *parsed* document, and the parser drops
    // comments. Every release that bumped a crate silently deleted them from Cargo.toml.
    const raw = [
      '[package]',
      'name    = "demo"',
      'version = "0.1.0"',
      '',
      '# UniFFI bindgen needs metadata symbols in the cdylib to generate bindings.',
      '# Stripping removes them. Keep symbols for the ffi crate.',
      '[profile.release.package.wvb-ffi]',
      'strip = "none"',
      '',
    ].join('\n');

    const out = bump(raw, '0.2.0');

    expect(out).toContain('# UniFFI bindgen needs metadata symbols in the cdylib to generate');
    expect(out).toContain('# Stripping removes them. Keep symbols for the ffi crate.');
    // Only the version line moves; the document is otherwise handed back untouched.
    expect(out).toBe(raw.replace('version = "0.1.0"', 'version = "0.2.0"'));
  });

  it('leaves a literal-string table key exactly as written', () => {
    // Regression: normalizing quotes turned `[target.'cfg(target_os = "android")'.dependencies]`
    // into invalid TOML (`unclosed table, expected ]`), failing `cargo publish` in the release job.
    const raw = [
      '[package]',
      'name    = "demo"',
      'version = "0.1.0"',
      '',
      '[target.\'cfg(target_os = "android")\'.dependencies]',
      'serde_json = { workspace = true }',
      '',
      "[target.'cfg(any())'.dependencies]",
      'alloc-stdlib = "=0.2.2"',
      '',
    ].join('\n');

    const out = bump(raw, '0.1.0-next.abc1234');

    expect(out).toContain('[target.\'cfg(target_os = "android")\'.dependencies]');
    expect(out).toContain("[target.'cfg(any())'.dependencies]");
    const reparsed = parseCargoToml(out) as unknown as { target: Record<string, unknown> };
    expect(Object.keys(reparsed.target)).toContain('cfg(target_os = "android")');
  });

  it('does not mistake `rust-version` for the package version', () => {
    const raw =
      '[package]\nname         = "demo"\nrust-version = "1.85.0"\nversion      = "0.1.0"\n';
    const out = editCargoTomlVersion(raw, Version.parse('0.2.0'));
    expect(out).toContain('rust-version = "1.85.0"');
    expect(out).toContain('version      = "0.2.0"');
  });

  it('throws instead of inventing a `[package]` version that is not there', () => {
    const raw = '[workspace]\nmembers = ["a"]\n';
    expect(() => editCargoTomlVersion(raw, Version.parse('0.2.0'))).toThrow(/\[package\]/);
  });
});

describe('editCargoTomlVersion (dependencies)', () => {
  const raw = [
    '[workspace.dependencies]',
    'wvb      = { version = "0.2.0", path = "./packages/core" }',
    'wvb-node = { version = "0.1.0", path = "./packages/node" }',
    'serde    = "1.0"',
    '',
    '[dependencies]',
    'wvb   = { workspace = true, features = ["full"] }',
    'bytes = "1"',
    '',
    "[target.'cfg(any())'.dependencies]",
    'wvb = "0.0.1"',
    '',
  ].join('\n');

  it('sets the version inside an inline table, leaving its siblings alone', () => {
    const out = bumpDep(raw, 'wvb', '0.3.0');
    expect(out).toContain('wvb      = { version = "0.3.0", path = "./packages/core" }');
    // The neighbouring entry must not be touched by a `wvb` edit.
    expect(out).toContain('wvb-node = { version = "0.1.0", path = "./packages/node" }');
  });

  it('leaves `{ workspace = true }` alone, having no version of its own', () => {
    const out = bumpDep(raw, 'wvb', '0.3.0');
    expect(out).toContain('wvb   = { workspace = true, features = ["full"] }');
  });

  it('does not reach into a `[target.*.dependencies]` table', () => {
    const out = bumpDep(raw, 'wvb', '0.3.0');
    expect(out).toContain('[target.\'cfg(any())\'.dependencies]\nwvb = "0.0.1"');
  });

  it('sets a bare string version', () => {
    const out = bumpDep(raw, 'serde', '2.0.0');
    expect(out).toContain('serde    = "2.0.0"');
  });

  it('pins a prerelease exactly, so dependents never resolve across channels', () => {
    const out = bumpDep(raw, 'wvb', '0.3.0-next.abc1234');
    expect(out).toContain('{ version = "=0.3.0-next.abc1234", path = "./packages/core" }');
  });

  it('is a no-op for a dependency the document does not declare', () => {
    expect(bumpDep(raw, 'not-a-dep', '9.9.9')).toBe(raw);
  });
});
