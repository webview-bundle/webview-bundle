import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { getNativeBindingPath, NATIVE_BINDING_PATH_ENV } from '../lib/native.js';

describe('NATIVE_BINDING_PATH_ENV', () => {
  it('is the napi loader override variable', () => {
    expect(NATIVE_BINDING_PATH_ENV).toBe('NAPI_RS_NATIVE_LIBRARY_PATH');
  });
});

describe('getNativeBindingPath', () => {
  it('reflects the override env var', () => {
    const original = process.env[NATIVE_BINDING_PATH_ENV];
    try {
      process.env[NATIVE_BINDING_PATH_ENV] = '/tmp/some/binding.node';
      expect(getNativeBindingPath()).toBe('/tmp/some/binding.node');
      delete process.env[NATIVE_BINDING_PATH_ENV];
      expect(getNativeBindingPath()).toBeUndefined();
      process.env[NATIVE_BINDING_PATH_ENV] = '';
      expect(getNativeBindingPath()).toBeUndefined();
    } finally {
      if (original == null) {
        delete process.env[NATIVE_BINDING_PATH_ENV];
      } else {
        process.env[NATIVE_BINDING_PATH_ENV] = original;
      }
    }
  });
});

// `@wvb/electron` bundles the native binaries and points `NAPI_RS_NATIVE_LIBRARY_PATH`
// at them. The napi-generated `binding.cjs` mishandles that branch out of the box (it
// assigns instead of returning, so the value is discarded), so we patch it after every
// `napi build`. Guard the patch here: without it the override silently does nothing.
describe('binding.cjs override branch', () => {
  it('returns the binary from NAPI_RS_NATIVE_LIBRARY_PATH instead of dropping it', () => {
    const binding = readFileSync(path.join(import.meta.dirname, '..', 'binding.cjs'), 'utf8');
    expect(binding).toContain('return require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH)');
    expect(binding).not.toContain(
      'nativeBinding = require(process.env.NAPI_RS_NATIVE_LIBRARY_PATH)'
    );
  });
});
