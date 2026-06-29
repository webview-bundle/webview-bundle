import { Buffer } from 'node:buffer';

export type IntegrityAlgorithm = 'sha256' | 'sha384' | 'sha512';

export interface IntegrityMakeConfig {
  algorithm?: IntegrityAlgorithm;
}
export type IntegrityMakeFn = (params: { data: Buffer }) => Promise<string>;
export type IntegrityMaker = IntegrityMakeConfig | IntegrityMakeFn;

export async function makeIntegrity(maker: IntegrityMaker, data: Buffer): Promise<string> {
  if (typeof maker === 'function') {
    return maker({ data });
  }
  const alg = maker?.algorithm ?? 'sha256';
  const hash = await crypto.subtle.digest({ name: hashAlg(alg) }, new Uint8Array(data));
  const hashBuf = Buffer.from(hash);
  return `${alg}:${hashBuf.toString('base64')}`;
}

function hashAlg(rasHash: IntegrityAlgorithm): string {
  switch (rasHash) {
    case 'sha256':
      return 'SHA-256';
    case 'sha384':
      return 'SHA-384';
    case 'sha512':
      return 'SHA-512';
  }
}
