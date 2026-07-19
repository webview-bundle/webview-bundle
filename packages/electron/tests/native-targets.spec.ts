import { describe, expect, it } from 'vitest';
import { NATIVE_TARGETS, nativeTargetForCurrentProcess } from '../src/native-targets.js';

describe('NATIVE_TARGETS', () => {
  it('covers exactly the desktop targets Electron can load', () => {
    expect(NATIVE_TARGETS.map(t => t.npmSuffix).sort()).toEqual(
      [
        'darwin-arm64',
        'darwin-x64',
        'linux-arm64-gnu',
        'linux-x64-gnu',
        'win32-arm64-msvc',
        'win32-ia32-msvc',
        'win32-x64-msvc',
      ].sort()
    );
  });

  it('excludes targets Electron cannot run (musl, android, freebsd, armv7)', () => {
    for (const target of NATIVE_TARGETS) {
      expect(target.triple).not.toMatch(/musl|android|freebsd|armv7/);
    }
  });

  it('has a unique (platform, arch) key per target', () => {
    const keys = NATIVE_TARGETS.map(t => `${t.platform}/${t.arch}`);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('names each binary `wvb-node.<suffix>.node`', () => {
    for (const target of NATIVE_TARGETS) {
      expect(target.file).toBe(`wvb-node.${target.npmSuffix}.node`);
    }
  });

  it('resolves the current process to one of the targets when supported', () => {
    const target = nativeTargetForCurrentProcess();
    if (target != null) {
      expect(target.platform).toBe(process.platform);
      expect(target.arch).toBe(process.arch);
    }
  });
});
