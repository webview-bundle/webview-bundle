import { Readable } from 'node:stream';
import { CloudFrontClient, CreateInvalidationCommand } from '@aws-sdk/client-cloudfront';
import { GetObjectCommand, PutObjectCommand, S3Client } from '@aws-sdk/client-s3';
import { sdkStreamMixin } from '@smithy/util-stream';
import { type AwsClientStub, mockClient } from 'aws-sdk-client-mock';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { awsRemoteDeployer } from './deployer.js';

describe('awsRemoteDeployer', () => {
  let s3Client: S3Client;
  let cloudFrontClient: CloudFrontClient;
  let s3Mock: AwsClientStub<S3Client>;
  let cloudFrontMock: AwsClientStub<CloudFrontClient>;

  beforeEach(() => {
    s3Client = new S3Client({ region: 'us-east-1' });
    cloudFrontClient = new CloudFrontClient({ region: 'us-east-1' });
    s3Mock = mockClient(s3Client);
    cloudFrontMock = mockClient(cloudFrontClient);
  });

  afterEach(() => {
    s3Mock.restore();
    cloudFrontMock.restore();
    s3Client.destroy();
    cloudFrontClient.destroy();
  });

  it('publishes signed update data with a current pointer and invalidates caches', async () => {
    s3Mock.onAnyCommand().rejects(new Error('Unexpected S3 request'));
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'channels/beta/updates/2/current.json' })
      .rejects({ name: 'NoSuchKey' });
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.json' })
      .callsFake(() => ({
        Body: sdkStreamMixin(
          Readable.from([
            JSON.stringify({ integrity: 'sha256-test', metadata: { platform: 'ios' } }),
          ])
        ),
      }));
    s3Mock.on(PutObjectCommand, { Bucket: 'bundles' }).resolves({});
    cloudFrontMock.onAnyCommand().rejects(new Error('Unexpected CloudFront request'));
    cloudFrontMock.on(CreateInvalidationCommand, { DistributionId: 'cdn' }).resolves({});
    const sign = vi.fn(async () => 'signed');
    const deployer = awsRemoteDeployer({
      bucket: 'bundles',
      s3Client,
      cloudFrontClient,
      invalidate: { distributionId: 'cdn' },
    });

    await deployer.deploy({
      bundles: [{ name: 'main', version: 'v1' }],
      runtimeVersion: 2,
      channel: 'beta',
      signatures: [{ id: 'test', sign }],
    });

    const pointerWrites = s3Mock.commandCalls(PutObjectCommand, {
      Bucket: 'bundles',
      Key: 'channels/beta/updates/2/current.json',
    });
    expect(pointerWrites).toHaveLength(1);
    const current = JSON.parse(pointerWrites[0]!.args[0].input.Body as string);
    expect(current).toEqual({ updateId: expect.any(String), currentAt: expect.any(String) });
    expect(Number.isFinite(Date.parse(current.currentAt))).toBe(true);

    const updateWrites = s3Mock.commandCalls(PutObjectCommand, {
      Bucket: 'bundles',
      Key: `channels/beta/updates/2/${current.updateId}.json`,
    });
    expect(updateWrites).toHaveLength(1);
    const update = JSON.parse(updateWrites[0]!.args[0].input.Body as string);
    expect(update.data).toMatchObject({
      id: current.updateId,
      runtimeVersion: 2,
      bundles: [
        { name: 'main', version: 'v1', integrity: 'sha256-test', metadata: { platform: 'ios' } },
      ],
    });
    expect(update.signatures).toEqual([{ id: 'test', alg: 'custom', sig: 'signed' }]);
    expect(sign).toHaveBeenCalledOnce();
    expect(
      cloudFrontMock.commandCalls(CreateInvalidationCommand, {
        DistributionId: 'cdn',
        InvalidationBatch: {
          CallerReference: current.updateId,
          Paths: { Quantity: 1, Items: ['/update*'] },
        },
      })
    ).toHaveLength(1);
  });

  it('keeps previously deployed bundles when updating a different bundle', async () => {
    const previous = {
      data: {
        id: '01936b84-8390-7000-8000-000000000000',
        createdAt: '2026-01-01T00:00:00.000Z',
        runtimeVersion: 1,
        bundles: [{ name: 'existing', version: 'v1' }],
      },
      signatures: [],
    };
    s3Mock.onAnyCommand().rejects(new Error('Unexpected S3 request'));
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'updates/1/current.json' })
      .callsFake(() => ({
        Body: sdkStreamMixin(
          Readable.from([
            JSON.stringify({ updateId: previous.data.id, currentAt: previous.data.createdAt }),
          ])
        ),
      }));
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: `updates/1/${previous.data.id}.json` })
      .callsFake(() => ({
        Body: sdkStreamMixin(Readable.from([JSON.stringify(previous)])),
      }));
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v2.json' })
      .rejects({ name: 'NoSuchKey' });
    s3Mock.on(PutObjectCommand, { Bucket: 'bundles' }).resolves({});

    await awsRemoteDeployer({ bucket: 'bundles', s3Client }).deploy({
      bundles: [{ name: 'main', version: 'v2' }],
    });

    const pointerWrites = s3Mock.commandCalls(PutObjectCommand, {
      Bucket: 'bundles',
      Key: 'updates/1/current.json',
    });
    expect(pointerWrites).toHaveLength(1);
    const current = JSON.parse(pointerWrites[0]!.args[0].input.Body as string);
    const updateWrites = s3Mock.commandCalls(PutObjectCommand, {
      Bucket: 'bundles',
      Key: `updates/1/${current.updateId}.json`,
    });
    expect(updateWrites).toHaveLength(1);
    const update = JSON.parse(updateWrites[0]!.args[0].input.Body as string);
    expect(update.data.bundles).toEqual([
      { name: 'existing', version: 'v1' },
      { name: 'main', version: 'v2' },
    ]);
  });

  it('does not publish or invalidate caches when bundle metadata cannot be read', async () => {
    const readError = new Error('Access denied to bundle metadata');
    s3Mock.onAnyCommand().rejects(new Error('Unexpected S3 request'));
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'updates/1/current.json' })
      .rejects({ name: 'NoSuchKey' });
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.json' })
      .rejects(readError);
    s3Mock.on(PutObjectCommand, { Bucket: 'bundles' }).resolves({});
    cloudFrontMock.onAnyCommand().rejects(new Error('Unexpected CloudFront request'));
    cloudFrontMock.on(CreateInvalidationCommand, { DistributionId: 'cdn' }).resolves({});

    await expect(
      awsRemoteDeployer({
        bucket: 'bundles',
        s3Client,
        cloudFrontClient,
        invalidate: { distributionId: 'cdn' },
      }).deploy({ bundles: [{ name: 'main', version: 'v1' }] })
    ).rejects.toBe(readError);
    expect(s3Mock.commandCalls(PutObjectCommand)).toHaveLength(0);
    expect(cloudFrontMock.commandCalls(CreateInvalidationCommand)).toHaveLength(0);
  });
});
