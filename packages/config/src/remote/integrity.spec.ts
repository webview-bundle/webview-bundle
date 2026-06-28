import { Buffer } from 'node:buffer';
import { describe, expect, it } from 'vitest';
import { makeIntegrity } from './integrity.js';

describe('makeIntegrity', () => {
  it('sha256 (default)', async () => {
    const data = Buffer.from('hello');
    const integrity = await makeIntegrity({}, data);
    expect(integrity).toEqual('sha256:LPJNul+wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ=');
  });

  it('sha384', async () => {
    const data = Buffer.from('hello');
    const integrity = await makeIntegrity({ algorithm: 'sha384' }, data);
    expect(integrity).toEqual(
      'sha384:WeF0h3dEjGnea4ANejO7+5/xtGPkQ1TDVTvNucZm+pASWjx5+QOXvfX2oT3oKGhP'
    );
  });

  it('sha512', async () => {
    const data = Buffer.from('hello');
    const integrity = await makeIntegrity({ algorithm: 'sha512' }, data);
    expect(integrity).toEqual(
      'sha512:m3HSJL1i83hdltRq0+o9czGb+8KJDKra4t/3JRlnPKcjI8PZm6XBHXx6zG4UuMXaDEZjR1wuXDre9G9zvN7AQw=='
    );
  });

  it('produces deterministic output for same input', async () => {
    const data = Buffer.from('same');
    const a = await makeIntegrity({ algorithm: 'sha384' }, data);
    const b = await makeIntegrity({ algorithm: 'sha384' }, data);
    expect(a).toBe(b);
  });
});
