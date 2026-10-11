import { GetObjectCommand } from '@aws-sdk/client-s3';
import { CurrentSchema, type Update, UpdateSchema } from '@wvb/remote-base';
import type { Context } from './types.js';
import { isNoSuchKeyError } from './utils.js';

export async function getUpdate(
  context: Context,
  options: { runtimeVersion: number; channel?: string }
): Promise<Update | null> {
  const prefix = `${options.channel == null ? '' : `channels/${options.channel}/`}updates/${options.runtimeVersion}`;
  const read = async (key: string) => {
    const output = await context.s3Client.send(
      new GetObjectCommand({ Bucket: context.bucketName, Key: key })
    );
    const raw = await output.Body?.transformToString('utf-8');
    if (raw == null) {
      throw new Error('Response body is empty');
    }
    return JSON.parse(raw);
  };
  try {
    const current = CurrentSchema.parse(await read(`${prefix}/current.json`));
    return UpdateSchema.parse(await read(`${prefix}/${current.updateId}.json`));
  } catch (error) {
    if (isNoSuchKeyError(error)) {
      return null;
    }
    throw error;
  }
}
