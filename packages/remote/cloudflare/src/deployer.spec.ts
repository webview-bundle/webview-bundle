import 'cloudflare/shims/web';
import { Readable } from 'node:stream';
import { GetObjectCommand, S3Client } from '@aws-sdk/client-s3';
import { sdkStreamMixin } from '@smithy/util-stream';
import { type AwsClientStub, mockClient } from 'aws-sdk-client-mock';
import Cloudflare, { type ClientOptions } from 'cloudflare';
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  type Mock,
  type MockInstance,
  vi,
} from 'vitest';
import { cloudflareRemoteDeployer } from './deployer.js';

describe('cloudflareRemoteDeployer', () => {
  let cloudflare: Cloudflare;
  let fetchMock: Mock<NonNullable<ClientOptions['fetch']>>;
  let put: MockInstance<Cloudflare['kv']['namespaces']['values']['update']>;
  let s3Client: S3Client;
  let s3Mock: AwsClientStub<S3Client>;

  beforeEach(() => {
    fetchMock = vi.fn(async () => {
      throw new Error('Unexpected Cloudflare request');
    });
    cloudflare = new Cloudflare({ apiToken: 'test', fetch: fetchMock, maxRetries: 0 });
    put = vi.spyOn(cloudflare.kv.namespaces.values, 'update').mockResolvedValue({});
    s3Client = new S3Client({ region: 'auto' });
    s3Mock = mockClient(s3Client);
    s3Mock.onAnyCommand().rejects(new Error('Unexpected R2 request'));
  });

  afterEach(() => {
    put.mockRestore();
    fetchMock.mockReset();
    s3Mock.restore();
    s3Client.destroy();
  });

  it('publishes signed update data and a current pointer for the requested channel and runtime', async () => {
    fetchMock.mockImplementation(async () => Response.json({}, { status: 404 }));
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.json' })
      .callsFake(() => ({
        Body: sdkStreamMixin(
          Readable.from([
            JSON.stringify({ integrity: 'sha256-test', metadata: { platform: 'ios' } }),
          ])
        ),
      }));
    const sign = vi.fn(async () => 'signed');

    await cloudflareRemoteDeployer({
      accountId: 'account',
      bucket: 'bundles',
      kvNamespaceId: 'namespace',
      cloudflare,
      s3Client,
    }).deploy({
      bundles: [{ name: 'main', version: 'v1' }],
      runtimeVersion: 2,
      channel: 'beta',
      metadata: { release: 'beta' },
      signatures: [{ id: 'test', sign }],
    });

    const pointerWrites = put.mock.calls.filter(
      ([namespace, key]) =>
        namespace === 'namespace' && key === 'channels/beta/updates/2/current.json'
    );
    expect(pointerWrites).toHaveLength(1);
    const current = JSON.parse(pointerWrites[0]![2].value);
    expect(Number.isFinite(Date.parse(current.currentAt))).toBe(true);
    const updateWrites = put.mock.calls.filter(
      ([namespace, key]) =>
        namespace === 'namespace' && key === `channels/beta/updates/2/${current.updateId}.json`
    );
    expect(updateWrites).toHaveLength(1);
    const update = JSON.parse(updateWrites[0]![2].value);
    expect(update.data).toMatchObject({
      id: current.updateId,
      runtimeVersion: 2,
      bundles: [
        { name: 'main', version: 'v1', integrity: 'sha256-test', metadata: { platform: 'ios' } },
      ],
      metadata: { release: 'beta' },
    });
    expect(update.etag).toEqual(expect.any(String));
    expect(update.signatures).toEqual([{ id: 'test', alg: 'custom', sig: 'signed' }]);
    expect(sign).toHaveBeenCalledOnce();
    expect(pointerWrites[0]![2].account_id).toBe('account');
    expect(updateWrites[0]![2].account_id).toBe('account');
  });

  it('merges bundle versions and preserves existing metadata when no replacement metadata is supplied', async () => {
    const previous = {
      data: {
        id: '01936b84-8390-7000-8000-000000000000',
        createdAt: '2026-01-01T00:00:00.000Z',
        runtimeVersion: 1,
        bundles: [
          { name: 'main', version: 'v1' },
          { name: 'existing', version: 'v1' },
        ],
        metadata: { release: 'stable' },
      },
      signatures: [],
    };
    const values = new Map<string, unknown>([
      [
        'updates/1/current.json',
        { updateId: previous.data.id, currentAt: previous.data.createdAt },
      ],
      [`updates/1/${previous.data.id}.json`, previous],
    ]);
    fetchMock.mockImplementation(async url => {
      const pathname = decodeURIComponent(new URL(String(url)).pathname);
      const prefix = '/client/v4/accounts/account/storage/kv/namespaces/namespace/values/';
      expect(pathname.startsWith(prefix)).toBe(true);
      const value = values.get(pathname.slice(prefix.length));
      return value == null ? Response.json({}, { status: 404 }) : Response.json(value);
    });
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v2.json' })
      .rejects({ name: 'NoSuchKey' });

    await cloudflareRemoteDeployer({
      accountId: 'account',
      bucket: 'bundles',
      kvNamespaceId: 'namespace',
      cloudflare,
      s3Client,
    }).deploy({ bundles: [{ name: 'main', version: 'v2' }] });

    const pointerWrites = put.mock.calls.filter(
      ([namespace, key]) => namespace === 'namespace' && key === 'updates/1/current.json'
    );
    expect(pointerWrites).toHaveLength(1);
    const current = JSON.parse(pointerWrites[0]![2].value);
    const updateWrites = put.mock.calls.filter(
      ([namespace, key]) =>
        namespace === 'namespace' && key === `updates/1/${current.updateId}.json`
    );
    expect(updateWrites).toHaveLength(1);
    const update = JSON.parse(updateWrites[0]![2].value);
    expect(update.data.bundles).toEqual([
      { name: 'existing', version: 'v1' },
      { name: 'main', version: 'v2' },
    ]);
    expect(update.data.metadata).toEqual({ release: 'stable' });
    expect(update.signatures).toEqual([]);
  });

  it('does not publish when Cloudflare rejects the current update lookup', async () => {
    fetchMock.mockImplementation(async () =>
      Response.json(
        { success: false, errors: [{ code: 10000, message: 'unauthorized' }] },
        { status: 403 }
      )
    );

    await expect(
      cloudflareRemoteDeployer({
        accountId: 'account',
        bucket: 'bundles',
        kvNamespaceId: 'namespace',
        cloudflare,
        s3Client,
      }).deploy({ bundles: [{ name: 'main', version: 'v1' }] })
    ).rejects.toMatchObject({ status: 403 });
    expect(put).not.toHaveBeenCalled();
    expect(s3Mock.commandCalls(GetObjectCommand)).toHaveLength(0);
  });

  it('does not publish when R2 rejects the bundle metadata lookup', async () => {
    fetchMock.mockImplementation(async () => Response.json({}, { status: 404 }));
    const readError = new Error('Access denied to bundle metadata');
    s3Mock
      .on(GetObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.json' })
      .rejects(readError);

    await expect(
      cloudflareRemoteDeployer({
        accountId: 'account',
        bucket: 'bundles',
        kvNamespaceId: 'namespace',
        cloudflare,
        s3Client,
      }).deploy({ bundles: [{ name: 'main', version: 'v1' }] })
    ).rejects.toBe(readError);
    expect(put).not.toHaveBeenCalled();
  });

  it('leaves the current pointer unchanged when saving the update fails', async () => {
    fetchMock.mockImplementation(async () => Response.json({}, { status: 404 }));
    const writeError = new Error('Update write failed');
    put.mockRejectedValue(writeError);

    await expect(
      cloudflareRemoteDeployer({
        accountId: 'account',
        bucket: 'bundles',
        kvNamespaceId: 'namespace',
        cloudflare,
        s3Client,
      }).deploy({ bundles: [] })
    ).rejects.toBe(writeError);
    expect(put.mock.calls.filter(([, key]) => key === 'updates/1/current.json')).toHaveLength(0);
  });
});
