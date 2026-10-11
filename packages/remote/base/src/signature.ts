import { Buffer } from 'node:buffer';
import { getSignatureValue, type ResolvedSignatureConfig } from '@wvb/config/remote';
import type { UpdateData, UpdateSignature } from './schema.js';
import { stringifyUpdateData } from './utils.js';

export async function generateUpdateDataSignatures(
  data: UpdateData,
  signatures: ResolvedSignatureConfig[]
): Promise<UpdateSignature[]> {
  const message = Buffer.from(stringifyUpdateData(data), 'utf8');
  const signed = await Promise.all(signatures.map(sig => getSignatureValue(sig, message)));
  return signed;
}
