import type { S3Client } from '@aws-sdk/client-s3';
import type { Callback, CloudFrontRequest } from 'hono/lambda-edge';

export interface Bindings {
  callback: Callback;
  request: CloudFrontRequest;
}

export interface Context {
  s3Client: S3Client;
  bucketName: string;
}
