import { Buffer } from 'node:buffer';
import type { SigningAlgorithmSpec } from '@aws-sdk/client-kms';
import type { SignatureSignFn } from '@wvb/config/remote';
import { type AwsKmsClientConfigLike, getKmsClient } from './utils.js';

export interface AwsKmsSignatureSignerConfig extends AwsKmsClientConfigLike {
  keyId: string;
  algorithm: SigningAlgorithmSpec;
}

export function awsKmsSignatureSigner(config: AwsKmsSignatureSignerConfig): SignatureSignFn {
  const { keyId, algorithm } = config;
  return async function sign(params) {
    const kms = await getKmsClient(config);
    const { SignCommand } = await import('@aws-sdk/client-kms');
    const output = await kms.send(
      new SignCommand({
        KeyId: keyId,
        Message: new Uint8Array(params.message),
        MessageType: 'DIGEST',
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
