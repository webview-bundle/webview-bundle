import { describe, expect, it } from 'vitest';
import { toBundleName, toProjectName, validateProjectName } from './project.js';

describe('toProjectName', () => {
  it('takes the basename of a path', () => {
    expect(toProjectName('/tmp/some/My-App')).toBe('my-app');
  });

  it('sanitizes and falls back for an empty result', () => {
    expect(toProjectName('.')).not.toBe('');
    expect(toProjectName('  ')).toBe('my-wvb-app');
  });
});

describe('toBundleName', () => {
  it('strips the npm scope', () => {
    expect(toBundleName('@acme/my-app')).toBe('my-app');
  });

  // The default hostname resolver splits on '.', so a dotted name would route app://a.b.wvb -> "a".
  it('removes dots so the bundle protocol resolves the whole name', () => {
    expect(toBundleName('acme.web')).toBe('acme-web');
    expect(toBundleName('my.app')).toBe('my-app');
  });

  it('removes characters the mobile route matcher rejects', () => {
    expect(toBundleName('my~app')).toBe('my-app');
    expect(toBundleName('-leading')).toBe('leading');
  });

  it('never returns an empty identifier', () => {
    expect(toBundleName('@scope/~')).toBe('app');
  });
});

describe('validateProjectName', () => {
  it('accepts scoped and plain lowercase names', () => {
    expect(validateProjectName('my-app')).toBeNull();
    expect(validateProjectName('@acme/my-app')).toBeNull();
  });

  it('rejects uppercase and empty names', () => {
    expect(validateProjectName('MyApp')).not.toBeNull();
    expect(validateProjectName('')).not.toBeNull();
  });
});
