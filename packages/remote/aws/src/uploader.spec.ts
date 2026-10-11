import { Buffer } from 'node:buffer';
import {
  CreateMultipartUploadCommand,
  HeadObjectCommand,
  PutObjectCommand,
  S3Client,
} from '@aws-sdk/client-s3';
import { type AwsClientStub, mockClient } from 'aws-sdk-client-mock';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BundleAlreadyUploadedError } from './errors.js';
import { awsRemoteUploader } from './uploader.js';

describe('awsRemoteUploader', () => {
  let s3Client: S3Client;
  let s3Mock: AwsClientStub<S3Client>;

  beforeEach(() => {
    s3Client = new S3Client({ region: 'us-east-1' });
    s3Mock = mockClient(s3Client);
  });

  afterEach(() => {
    s3Mock.restore();
    s3Client.destroy();
  });

  it('uploads the bundle and version data using the keys read by deployment and Lambda', async () => {
    s3Mock.onAnyCommand().rejects(new Error('Unexpected S3 request'));
    s3Mock
      .on(HeadObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.wvb' })
      .rejects({ name: 'NotFound' });
    s3Mock.on(PutObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.wvb' }).resolves({});
    s3Mock.on(PutObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.json' }).resolves({});
    const uploader = awsRemoteUploader({ bucket: 'bundles', s3Client });

    await uploader.upload({
      name: 'main',
      version: 'v1',
      bundle: Buffer.from('bundle'),
      versionData: { integrity: 'sha256-test' },
    });

    expect(
      s3Mock.commandCalls(PutObjectCommand, {
        Bucket: 'bundles',
        Key: 'bundles/main/v1.wvb',
        Body: Buffer.from('bundle'),
        ContentType: 'application/webview-bundle',
        Metadata: { 'wvb-bundle-name': 'main', 'wvb-bundle-version': 'v1' },
      })
    ).toHaveLength(1);
    expect(
      s3Mock.commandCalls(PutObjectCommand, {
        Bucket: 'bundles',
        Key: 'bundles/main/v1.json',
        Body: JSON.stringify({ integrity: 'sha256-test' }),
        ContentType: 'application/json',
      })
    ).toHaveLength(1);
  });

  it('rejects an existing version without starting an upload', async () => {
    s3Mock.onAnyCommand().rejects(new Error('Unexpected S3 request'));
    s3Mock.on(HeadObjectCommand, { Bucket: 'bundles', Key: 'bundles/main/v1.wvb' }).resolves({});

    await expect(
      awsRemoteUploader({ bucket: 'bundles', s3Client }).upload({
        name: 'main',
        version: 'v1',
        bundle: Buffer.from('bundle'),
        versionData: { integrity: 'sha256-test' },
      })
    ).rejects.toBeInstanceOf(BundleAlreadyUploadedError);
    expect(s3Mock.commandCalls(PutObjectCommand)).toHaveLength(0);
    expect(s3Mock.commandCalls(CreateMultipartUploadCommand)).toHaveLength(0);
  });
});
