import { describe, expect, it } from 'vitest';
import { parseExpectSignatureHeader } from './signature.js';

describe('parseExpectSignatureHeader', () => {
  it('returns null when the header is absent', () => {
    expect(parseExpectSignatureHeader(undefined)).toBeNull();
  });

  it('parses an SFV dictionary', () => {
    expect(parseExpectSignatureHeader('key_id="release", alg="ed25519"')).toEqual({
      keyId: 'release',
      alg: 'ed25519',
    });
  });

  it('accepts member order, whitespace, parameters, and extra members', () => {
    expect(
      parseExpectSignatureHeader(
        ' alg="ed25519";version=1, extra="ignored", key_id="release";primary'
      )
    ).toEqual({
      keyId: 'release',
      alg: 'ed25519',
    });
  });

  it.each([
    ['', 'missing or invalid "key_id" member'],
    ['alg="ed25519"', 'missing or invalid "key_id" member'],
    ['key_id="release"', 'missing or invalid "alg" member'],
    ['key_id=release, alg="ed25519"', 'missing or invalid "key_id" member'],
    ['key_id="release", alg=ed25519', 'missing or invalid "alg" member'],
    ['key_id="release", alg="ed25519",', 'malformed SFV dictionary'],
  ])('rejects an invalid header: %s', (header, message) => {
    expect(() => parseExpectSignatureHeader(header)).toThrow(message);
  });
});
