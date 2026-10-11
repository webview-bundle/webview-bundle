import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';
import { KMSClient } from '@aws-sdk/client-kms';
import { expect, it, vi } from 'vitest';
import { awsKmsSignatureSigner } from './signature.js';

it('prehashes large RSA updates before sending a digest to KMS', async () => {
  const kmsClient = new KMSClient({ region: 'us-east-1' });
  using send = vi
    .spyOn(kmsClient, 'send')
    .mockImplementation(async () => ({ Signature: new Uint8Array([1, 2, 3]) }));
  const sign = awsKmsSignatureSigner({
    kmsClient,
    keyId: 'key',
    algorithm: 'RSASSA_PKCS1_V1_5_SHA_256',
  });
  const message = Buffer.alloc(5000, 'a');
  expect(await sign({ message })).toBe('AQID');
  expect(send.mock.calls[0]![0].input).toMatchObject({
    MessageType: 'DIGEST',
    Message: new Uint8Array(createHash('sha256').update(message).digest()),
  });
});

it('preserves RAW input for Ed25519, which requires an unhashed message', async () => {
  const kmsClient = new KMSClient({ region: 'us-east-1' });
  using send = vi
    .spyOn(kmsClient, 'send')
    .mockImplementation(async () => ({ Signature: new Uint8Array([1]) }));
  await awsKmsSignatureSigner({ kmsClient, keyId: 'key', algorithm: 'ED25519_SHA_512' })({
    message: Buffer.from('update'),
  });
  expect(send.mock.calls[0]![0].input).toMatchObject({
    MessageType: 'RAW',
    Message: new Uint8Array(Buffer.from('update')),
  });
});
