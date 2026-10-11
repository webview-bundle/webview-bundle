import { Buffer } from 'node:buffer';

export type SigningKeyFormat = 'raw' | 'pkcs8' | 'spki' | 'jwk';

export type SignatureSigningKeyConfig =
  | {
      format: 'jwk';
      data: JsonWebKey;
    }
  | {
      format: Exclude<SigningKeyFormat, 'jwk'>;
      data: Buffer;
    };

export type SignatureSignConfig =
  | {
      algorithm: 'ecdsa-secp256r1' | 'ecdsa-secp384r1';
      key: SignatureSigningKeyConfig;
    }
  | {
      algorithm: 'rsa-pkcs1-v1_5-sha256';
      key: SignatureSigningKeyConfig;
    }
  | {
      algorithm: 'rsa-pss-sha256';
      saltLength?: number;
      key: SignatureSigningKeyConfig;
    }
  | {
      algorithm: 'ed25519';
      key: SignatureSigningKeyConfig;
    };

export type SignatureAlgorithm = SignatureSignConfig['algorithm'];
export type SignatureSignFn = (params: { message: Buffer }) => Promise<string>;
export type SignatureSigner = SignatureSignConfig | SignatureSignFn;

export async function signSignature(signer: SignatureSigner, message: Buffer): Promise<string> {
  if (typeof signer === 'function') {
    return await signer({ message });
  }
  const { key } = signer;
  const signingKey =
    key.format === 'jwk'
      ? await crypto.subtle.importKey(key.format, key.data, importKeyAlg(signer), true, ['sign'])
      : await crypto.subtle.importKey(
          key.format,
          new Uint8Array(key.data),
          importKeyAlg(signer),
          true,
          ['sign']
        );
  const signed = await crypto.subtle.sign(signAlg(signer), signingKey, new Uint8Array(message));
  const signedBuf = Buffer.from(signed);
  return signedBuf.toString('base64');
}

export interface SignatureConfig {
  /** @default default */
  id?: string;
  sign: SignatureSigner;
}
export type ResolvedSignatureConfig = Required<SignatureConfig>;

export async function getSignatureValue(
  sig: ResolvedSignatureConfig,
  message: Buffer
): Promise<{
  id: string;
  alg: string;
  sig: string;
}> {
  const signature = await signSignature(sig.sign, message);
  const alg = formatAlgorithm(sig.sign);

  return {
    id: sig.id,
    alg,
    sig: signature,
  };
}

function formatAlgorithm(signer: SignatureSigner): string {
  if (typeof signer === 'function') {
    return 'custom';
  }
  return signer.algorithm;
}

function importKeyAlg(
  config: SignatureSignConfig
): AlgorithmIdentifier | RsaHashedImportParams | EcKeyAlgorithm {
  switch (config.algorithm) {
    case 'ecdsa-secp256r1':
      return {
        name: 'ECDSA',
        namedCurve: 'P-256',
      };
    case 'ecdsa-secp384r1':
      return {
        name: 'ECDSA',
        namedCurve: 'P-384',
      };
    case 'ed25519':
      return { name: 'Ed25519' };
    case 'rsa-pkcs1-v1_5-sha256':
      return {
        name: 'RSASSA-PKCS1-v1_5',
        hash: 'SHA-256',
      };
    case 'rsa-pss-sha256':
      return {
        name: 'RSA-PSS',
        hash: 'SHA-256',
      };
  }
}

function signAlg(config: SignatureSignConfig): AlgorithmIdentifier | RsaPssParams | EcdsaParams {
  switch (config.algorithm) {
    case 'ecdsa-secp256r1':
    case 'ecdsa-secp384r1':
      return {
        name: 'ECDSA',
        hash: 'SHA-256',
      };
    case 'ed25519':
      return { name: 'Ed25519' };
    case 'rsa-pkcs1-v1_5-sha256':
      return { name: 'RSASSA-PKCS1-v1_5' };
    case 'rsa-pss-sha256':
      return {
        name: 'RSA-PSS',
        saltLength: 32,
      };
  }
}
