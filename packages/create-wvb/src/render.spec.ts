import { describe, expect, it } from 'vitest';
import { mergeManifest, type RenderContext, substitute } from './render.js';

const ctx: RenderContext = {
  projectName: 'my-app',
  bundleName: 'my-app',
  pm: 'pnpm',
  pmRun: 'pnpm',
  versions: { '@wvb/cli': '0.1.0', '@wvb/electron': '0.2.0-next.abc' },
};

describe('substitute', () => {
  it('replaces the name tokens', () => {
    expect(substitute('{{projectName}} / {{bundleName}}', ctx, 't')).toBe('my-app / my-app');
  });

  it('replaces package-manager tokens', () => {
    expect(substitute('{{pm}} install && {{pmRun}} dev', ctx, 't')).toBe(
      'pnpm install && pnpm dev'
    );
  });

  it('resolves wvbVersion through the range rule', () => {
    expect(substitute('"{{wvbVersion:@wvb/cli}}"', ctx, 't')).toBe('"^0.1.0"');
    expect(substitute('"{{wvbVersion:@wvb/electron}}"', ctx, 't')).toBe('"0.2.0-next.abc"');
  });

  it('throws on an unknown token rather than emitting it literally', () => {
    expect(() => substitute('{{nope}}', ctx, 'a/b.ts')).toThrow(
      /a\/b\.ts: unknown token "\{\{nope\}\}"/
    );
  });

  it('throws on an unresolved package rather than guessing a version', () => {
    expect(() => substitute('{{wvbVersion:@wvb/ghost}}', ctx, 'a/b.ts')).toThrow(
      /no resolved version for "@wvb\/ghost"/
    );
  });

  it('throws when wvbVersion has no package', () => {
    expect(() => substitute('{{wvbVersion}}', ctx, 'a/b.ts')).toThrow(/needs a package/);
  });
});

describe('mergeManifest', () => {
  it('lets a later layer win for scalars', () => {
    expect(mergeManifest({ name: 'a', version: '1' }, { name: 'b' })).toEqual({
      name: 'b',
      version: '1',
    });
  });

  it('merges dependency maps key-wise so an overlay contributes only its delta', () => {
    const merged = mergeManifest(
      { devDependencies: { electron: '^38.0.0', vite: '^7.0.0' } },
      { devDependencies: { '@electron-forge/cli': '^7.11.0' } }
    );
    expect(merged.devDependencies).toEqual({
      electron: '^38.0.0',
      vite: '^7.0.0',
      '@electron-forge/cli': '^7.11.0',
    });
  });

  it('merges scripts key-wise and lets the overlay override one', () => {
    const merged = mergeManifest(
      { scripts: { dev: 'base-dev', build: 'base-build' } },
      { scripts: { dev: 'overlay-dev', package: 'forge package' } }
    );
    expect(merged.scripts).toEqual({
      dev: 'overlay-dev',
      build: 'base-build',
      package: 'forge package',
    });
  });

  it('replaces arrays rather than concatenating them', () => {
    expect(mergeManifest({ files: ['a', 'b'] }, { files: ['c'] })).toEqual({ files: ['c'] });
  });
});
