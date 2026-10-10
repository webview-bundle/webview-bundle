import { decodeDictionary, Item } from '@shogo82148/sfv';

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
    throw new TypeError('Invalid wvb-expect-signature header: malformed SFV dictionary', {
      cause,
    });
  }

  return {
    keyId: getStringMember(dictionary, 'key_id'),
    alg: getStringMember(dictionary, 'alg'),
  };
}

function getStringMember(dictionary: ReturnType<typeof decodeDictionary>, key: string): string {
  const member = dictionary.get(key);
  if (!(member instanceof Item) || typeof member.value !== 'string') {
    throw new TypeError(`Invalid wvb-expect-signature header: missing or invalid "${key}" member`);
  }
  return member.value;
}
