import * as pulumi from '@pulumi/pulumi';
import { expect, it } from 'vitest';
import { WebviewBundleRemoteProvider } from './provider.js';

it('creates a published edge Lambda and forwards update protocol headers', async () => {
  const resources: pulumi.runtime.MockResourceArgs[] = [];
  pulumi.runtime.setMocks(
    {
      newResource(args) {
        resources.push(args);
        return {
          id: args.name,
          state: {
            ...args.inputs,
            arn: `arn:${args.name}`,
            qualifiedArn: `arn:${args.name}:1`,
            domainName: 'cdn.example.com',
            bucketRegionalDomainName: 'bucket.s3.amazonaws.com',
            bucketDomainName: 'bucket.s3.amazonaws.com',
            iamArn: 'arn:oai',
            cloudfrontAccessIdentityPath: 'origin-access-identity/cloudfront/oai',
          },
        };
      },
      call: () => ({ json: '{}' }),
    },
    'project',
    'stack',
    false
  );
  const remote = new WebviewBundleRemoteProvider('test', {
    region: 'ap-northeast-2',
    lambdaOriginRequest: {
      code: new pulumi.asset.AssetArchive({
        'origin-request.mjs': new pulumi.asset.StringAsset('export const handler = () => {};'),
      }),
    },
  });
  await new Promise<void>(resolve => {
    pulumi.all([remote.lambdaOriginRequestArn, remote.cloudfrontDistributionId]).apply(() => {
      resolve();
    });
  });
  const lambda = resources.find(resource => resource.type === 'aws:lambda/function:Function')!;
  expect(lambda.inputs).toMatchObject({
    publish: true,
    runtime: 'nodejs22.x',
    handler: 'origin-request.handler',
  });
  expect(lambda.inputs.environment).toBeUndefined();
  const distribution = resources.find(
    resource => resource.type === 'aws:cloudfront/distribution:Distribution'
  )!;
  expect(distribution.inputs.defaultCacheBehavior.forwardedValues.headers).toContain(
    'wvb-runtime-version'
  );
  expect(distribution.inputs.defaultCacheBehavior.forwardedValues.headers).toContain(
    'wvb-update-channel'
  );
  expect(
    resources.some(
      resource => resource.type === 'pulumi:providers:aws' && resource.inputs.region === 'us-east-1'
    )
  ).toBe(true);
});
