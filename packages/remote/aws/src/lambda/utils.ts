import { NoSuchKey } from '@aws-sdk/client-s3';

export function isNoSuchKeyError(e: unknown): boolean {
  return (
    e instanceof NoSuchKey ||
    (e != null && typeof e === 'object' && (e as any).name === 'NoSuchKey')
  );
}
