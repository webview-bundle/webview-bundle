import * as TOML from '@ltd/j-toml';
import { describe, expect, it } from 'vitest';
import { editCargoTomlVersion, formatCargoToml, parseCargoToml } from './cargo-toml.ts';
import { Version } from './version.ts';

function bump(raw: string, version: string): string {
  const toml = parseCargoToml(raw);
  editCargoTomlVersion(toml, Version.parse(version));
  return formatCargoToml(toml);
}

describe('formatCargoToml', () => {
  it('bumps the package version', () => {
    const out = bump('[package]\nname = "demo"\nversion = "0.1.0"\n', '0.1.0-next.abc1234');
    expect(out).toContain('version = "0.1.0-next.abc1234"');
    expect(() => TOML.parse(out)).not.toThrow();
  });

  it('keeps a literal-string table key that embeds a double quote valid', () => {
    // Regression: the naive `'` -> `"` replacement turned
    // `[target.'cfg(target_os = "android")'.dependencies]` into invalid TOML
    // (`unclosed table, expected ]`), failing `cargo publish` in the release job.
    const raw = [
      '[package]',
      'name = "demo"',
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

    // The literal key with an embedded `"` must stay a literal (single-quoted) string.
    expect(out).toContain('[target.\'cfg(target_os = "android")\'.dependencies]');
    // A literal key with no embedded `"` is normalized to a basic (double-quoted) string.
    expect(out).toContain('[target."cfg(any())".dependencies]');
    // Above all, the result must be parseable TOML.
    const reparsed = parseCargoToml(out) as unknown as { target: Record<string, unknown> };
    expect(Object.keys(reparsed.target)).toContain('cfg(target_os = "android")');
  });

  it('does not touch apostrophes inside basic strings', () => {
    const out = bump(
      '[package]\nname = "demo"\nversion = "0.1.0"\ndescription = "it\'s fine"\n',
      '0.1.0-next.abc1234'
    );
    expect(out).toContain('description = "it\'s fine"');
    expect(() => TOML.parse(out)).not.toThrow();
  });
});
