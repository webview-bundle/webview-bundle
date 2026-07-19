import { describe, expect, it } from 'vitest';

const ENV = 'NAPI_RS_NATIVE_LIBRARY_PATH';

// `NAPI_RS_NATIVE_LIBRARY_PATH` is the override every @napi-rs/cli loader reads, not
// just @wvb/node's. If native.ts left it set, any other napi-rs native module in the
// Electron process (or a spawned child) would load @wvb/node's binary as its own.
// Guard that native.ts restores it.
describe('native.ts env isolation', () => {
  it('does not leave NAPI_RS_NATIVE_LIBRARY_PATH set after import', async () => {
    const had = Object.hasOwn(process.env, ENV);
    const previous = process.env[ENV];
    delete process.env[ENV];
    try {
      await import('../src/native.js');
      expect(process.env[ENV]).toBeUndefined();
    } finally {
      if (had) {
        process.env[ENV] = previous;
      } else {
        delete process.env[ENV];
      }
    }
  });
});
