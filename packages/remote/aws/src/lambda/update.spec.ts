import { NoSuchKey, S3Client } from '@aws-sdk/client-s3';
import { describe, expect, it, vi } from 'vitest';
import { getUpdate } from './update.js';

describe('getUpdate', () => {
  it('reads the current update for the requested runtime and channel', async () => {
    const s3Client = new S3Client({ region: 'us-east-1' });
    const update = {
      data: {
        id: '01936b84-8390-7000-8000-000000000000',
        createdAt: '2026-01-01T00:00:00.000Z',
        runtimeVersion: 2,
        bundles: [],
      },
      signatures: [],
    };
    using send = vi
      .spyOn(s3Client, 'send')
      .mockImplementationOnce(async () => ({
        Body: {
          transformToString: async () =>
            JSON.stringify({ updateId: update.data.id, currentAt: update.data.createdAt }),
        },
      }))
      .mockImplementationOnce(async () => ({
        Body: { transformToString: async () => JSON.stringify(update) },
      }));
    expect(
      await getUpdate({ s3Client, bucketName: 'bundles' }, { runtimeVersion: 2, channel: 'beta' })
    ).toEqual(update);
    expect(send.mock.calls.map(([command]) => command.input)).toEqual([
      { Bucket: 'bundles', Key: 'channels/beta/updates/2/current.json' },
      { Bucket: 'bundles', Key: `channels/beta/updates/2/${update.data.id}.json` },
    ]);
  });

  it('returns no update when the current pointer does not exist', async () => {
    const s3Client = new S3Client({ region: 'us-east-1' });
    using send = vi
      .spyOn(s3Client, 'send')
      .mockRejectedValue(new NoSuchKey({ $metadata: {}, message: 'missing' }));
    expect(await getUpdate({ s3Client, bucketName: 'bundles' }, { runtimeVersion: 1 })).toBeNull();
    expect(send).toHaveBeenCalledOnce();
  });

  it('propagates access failures instead of treating them as an empty remote', async () => {
    const s3Client = new S3Client({ region: 'us-east-1' });
    using send = vi.spyOn(s3Client, 'send').mockRejectedValue(new Error('AccessDenied'));
    await expect(
      getUpdate({ s3Client, bucketName: 'bundles' }, { runtimeVersion: 1 })
    ).rejects.toThrow('AccessDenied');
    expect(send).toHaveBeenCalledOnce();
  });
});
