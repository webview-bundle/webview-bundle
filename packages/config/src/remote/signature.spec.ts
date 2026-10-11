import { Buffer } from 'node:buffer';
import { describe, expect, it } from 'vitest';
import { type SignatureAlgorithm, signSignature } from './signature.js';

const message = Buffer.from('test message');

async function exportPrivateKey(keyPair: CryptoKeyPair): Promise<Buffer> {
  const pkcs8 = await crypto.subtle.exportKey('pkcs8', keyPair.privateKey);
  return Buffer.from(pkcs8);
}

async function verifySignature(
  algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams,
  publicKey: CryptoKey,
  signature: string
): Promise<boolean> {
  return crypto.subtle.verify(
    algorithm,
    publicKey,
    new Uint8Array(Buffer.from(signature, 'base64')),
    new Uint8Array(message)
  );
}

describe('signSignature', () => {
  it.each([
    ['ecdsa-secp256r1', 'P-256'],
    ['ecdsa-secp384r1', 'P-384'],
  ] satisfies [SignatureAlgorithm, string][])('%s', async (algorithm, namedCurve) => {
    const keyPair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve }, true, [
      'sign',
      'verify',
    ]);
    const signature = await signSignature(
      {
        algorithm,
        key: {
          format: 'pkcs8',
          data: await exportPrivateKey(keyPair),
        },
      },
      message
    );

    const verified = await verifySignature(
      { name: 'ECDSA', hash: 'SHA-256' },
      keyPair.publicKey,
      signature
    );
    expect(verified).toBe(true);
  });

  it('ed25519', async () => {
    const keyPair = await crypto.subtle.generateKey({ name: 'Ed25519' }, true, ['sign', 'verify']);
    const signature = await signSignature(
      {
        algorithm: 'ed25519',
        key: {
          format: 'pkcs8',
          data: await exportPrivateKey(keyPair),
        },
      },
      message
    );

    const verified = await verifySignature({ name: 'Ed25519' }, keyPair.publicKey, signature);
    expect(verified).toBe(true);
  });

  it('rsa-pkcs1-v1_5-sha256', async () => {
    const keyPair = await crypto.subtle.generateKey(
      {
        name: 'RSASSA-PKCS1-v1_5',
        modulusLength: 2048,
        publicExponent: new Uint8Array([1, 0, 1]),
        hash: 'SHA-256',
      },
      true,
      ['sign', 'verify']
    );
    const signature = await signSignature(
      {
        algorithm: 'rsa-pkcs1-v1_5-sha256',
        key: {
          format: 'pkcs8',
          data: await exportPrivateKey(keyPair),
        },
      },
      message
    );

    const verified = await verifySignature(
      { name: 'RSASSA-PKCS1-v1_5' },
      keyPair.publicKey,
      signature
    );
    expect(verified).toBe(true);
  });

  it('rsa-pss-sha256', async () => {
    const keyPair = await crypto.subtle.generateKey(
      {
        name: 'RSA-PSS',
        modulusLength: 2048,
        publicExponent: new Uint8Array([1, 0, 1]),
        hash: 'SHA-256',
      },
      true,
      ['sign', 'verify']
    );
    const signature = await signSignature(
      {
        algorithm: 'rsa-pss-sha256',
        key: {
          format: 'pkcs8',
          data: await exportPrivateKey(keyPair),
        },
      },
      message
    );

    const verified = await verifySignature(
      { name: 'RSA-PSS', saltLength: 32 },
      keyPair.publicKey,
      signature
    );
    expect(verified).toBe(true);
  });
});
