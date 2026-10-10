import { S3Client } from '@aws-sdk/client-s3';
import type { CloudFrontRequestEvent, Context } from 'aws-lambda';
import { expect, it, vi } from 'vitest';
import { webviewBundleRemoteHandler } from './handler.js';

it('rewrites a download to the uploaded S3 key and forwards the original request', async () => {
  const s3Client = new S3Client({ region: 'us-east-1' });
  using send = vi.spyOn(s3Client, 'send');
  const onError = vi.fn();
  const handler = webviewBundleRemoteHandler({ bucketName: 'bundles', s3Client, onError });
  const event: CloudFrontRequestEvent = {
    Records: [
      {
        cf: {
          config: {
            distributionDomainName: 'cdn.example.com',
            distributionId: 'distribution',
            eventType: 'origin-request',
            requestId: 'request',
          },
          request: {
            clientIp: '127.0.0.1',
            method: 'GET',
            uri: '/bundles/main/v1',
            querystring: '',
            headers: {},
          },
        },
      },
    ],
  };
  const callback = vi.fn();
  const response = await handler(event, {} as Context, callback);
  expect(response).toEqual(expect.objectContaining({ uri: '/bundles/main/v1.wvb' }));
  expect(callback).toHaveBeenCalledExactlyOnceWith(null, event.Records[0]!.cf.request);
  expect(send).not.toHaveBeenCalled();
  expect(onError).not.toHaveBeenCalled();
});
