import { Buffer } from 'node:buffer';
import { createHash } from 'node:crypto';
import type { SigningAlgorithmSpec } from '@aws-sdk/client-kms';
import type { SignatureSignFn } from '@wvb/config/remote';
import { type AwsKmsClientConfigLike, getKmsClient } from './sdk.js';

export interface AwsKmsSignatureSignerConfig extends AwsKmsClientConfigLike {
  keyId: string;
  algorithm: SigningAlgorithmSpec;
}

export function awsKmsSignatureSigner(config: AwsKmsSignatureSignerConfig): SignatureSignFn {
  const { keyId, algorithm } = config;
  return async function sign(params) {
    const kms = await getKmsClient(config);
    const { SignCommand } = await import('@aws-sdk/client-kms');
    // Prehash RSA/ECDSA updates so KMS's 4096-byte request limit does not limit update size.
    const hash =
      algorithm === 'ED25519_PH_SHA_512'
        ? '512'
        : /^(?:RSASSA_.+|ECDSA)_SHA_(256|384|512)$/.exec(algorithm)?.[1];
    const message =
      hash == null ? params.message : createHash(`sha${hash}`).update(params.message).digest();
    const output = await kms.send(
      new SignCommand({
        KeyId: keyId,
        Message: new Uint8Array(message),
        MessageType: hash == null ? 'RAW' : 'DIGEST',
        SigningAlgorithm: algorithm,
      })
    );
    const { Signature: signature } = output;
    if (signature == null) {
      throw new Error('Signature not found in KMS response');
    }
    const encoded = Buffer.from(signature).toString('base64');
    return encoded;
  };
}
