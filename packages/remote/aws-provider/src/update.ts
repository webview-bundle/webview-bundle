import { GetObjectCommand } from '@aws-sdk/client-s3';
import type { Update } from '@wvb/remote-base-provider';
import type { Context } from './types.js';
import { isNoSuchKeyError } from './utils.js';

interface Options {
  channel?: string;
}

export async function getUpdate(context: Context, options?: Options): Promise<Update | null> {
  try {
    const key =
      options?.channel != null ? `channels/${options.channel}/update.json` : 'update.json';
    const output = await context.s3Client.send(
      new GetObjectCommand({
        Bucket: context.bucketName,
        Key: key,
      })
    );
    const raw = await output.Body?.transformToString('utf-8');
    if (raw == null) {
      throw new Error('Response body is empty');
    }
    return JSON.parse(raw);
  } catch (e) {
    if (isNoSuchKeyError(e)) {
      return null;
    }
    throw e;
  }
}
