import { decodeDictionary, Item } from '@shogo82148/sfv';
import type { UpdateSignature } from '../schema.js';
import { Exception } from './exception.js';

interface ExpectSignature {
  keyId: string;
  alg: string;
}

export function parseExpectSignatureHeader(
  headerValue: string | undefined
): ExpectSignature | null {
  if (headerValue == null) {
    return null;
  }

  let dictionary: ReturnType<typeof decodeDictionary>;
  try {
    dictionary = decodeDictionary(headerValue);
  } catch (cause) {
    throw new Exception(
      'invalid_expect_signature',
      {
        requestedExpectSignature: headerValue,
        detail: 'malformed SFV dictionary',
      },
      { cause }
    );
  }

  return {
    keyId: getStringMember(headerValue, dictionary, 'key_id'),
    alg: getStringMember(headerValue, dictionary, 'alg'),
  };
}

function getStringMember(
  headerValue: string,
  dictionary: ReturnType<typeof decodeDictionary>,
  key: string
): string {
  const member = dictionary.get(key);
  if (!(member instanceof Item) || typeof member.value !== 'string') {
    throw new Exception('invalid_expect_signature', {
      requestedExpectSignature: headerValue,
      detail: `missing or invalid "${key}" member`,
    });
  }
  return member.value;
}

export function serializeSignature(signature: UpdateSignature): string {
  return `key_id="${signature.id}", alg="${signature.alg}", sig="${signature.sig}"`;
}
