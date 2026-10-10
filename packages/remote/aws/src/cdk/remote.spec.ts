import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { App, CfnParameter, Duration, Stack } from 'aws-cdk-lib';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { HttpVersion, PriceClass } from 'aws-cdk-lib/aws-cloudfront';
import { ManagedPolicy, PolicyStatement, Role, ServicePrincipal } from 'aws-cdk-lib/aws-iam';
import { Code, Runtime } from 'aws-cdk-lib/aws-lambda';
import { expect, it } from 'vitest';
import { WebviewBundleRemote } from './index.js';

it('creates a private remote with a published Lambda version and isolated update cache keys', () => {
  using temporary = {
    path: mkdtempSync(path.join(tmpdir(), 'wvb-cdk-')),
    [Symbol.dispose]() {
      rmSync(this.path, { recursive: true, force: true });
    },
  };
  const app = new App({ outdir: temporary.path, autoSynth: false });
  const stack = new Stack(app, 'Remote', { env: { account: '123456789012', region: 'us-east-1' } });
  const remote = new WebviewBundleRemote(stack, 'Bundles', { bucketName: 'my-wvb-bundles' });
  const template = Template.fromStack(stack);

  template.hasResourceProperties('AWS::S3::Bucket', {
    BucketName: 'my-wvb-bundles',
    PublicAccessBlockConfiguration: {
      BlockPublicAcls: true,
      BlockPublicPolicy: true,
      IgnorePublicAcls: true,
      RestrictPublicBuckets: true,
    },
  });
  template.hasResourceProperties('AWS::Lambda::Function', {
    Runtime: 'nodejs22.x',
    Handler: 'origin-request.handler',
    Timeout: 30,
    Environment: Match.absent(),
    Layers: Match.absent(),
    Code: { S3Bucket: Match.anyValue(), S3Key: Match.anyValue() },
  });
  template.resourceCountIs('AWS::Lambda::Version', 1);
  template.hasResource('AWS::Lambda::Version', {
    DeletionPolicy: 'Retain',
    UpdateReplacePolicy: 'Retain',
  });
  template.hasResourceProperties('AWS::CloudFront::OriginAccessControl', {
    OriginAccessControlConfig: Match.objectLike({
      OriginAccessControlOriginType: 's3',
      SigningBehavior: 'always',
    }),
  });
  template.hasResourceProperties('AWS::CloudFront::CachePolicy', {
    CachePolicyConfig: Match.objectLike({
      MinTTL: 0,
      DefaultTTL: 0,
      ParametersInCacheKeyAndForwardedToOrigin: Match.objectLike({
        HeadersConfig: {
          HeaderBehavior: 'whitelist',
          Headers: [
            'wvb-update-protocol-version',
            'wvb-runtime-version',
            'wvb-update-channel',
            'wvb-expect-signature',
            'if-none-match',
          ],
        },
      }),
    }),
  });
  template.hasResourceProperties('AWS::CloudFront::Distribution', {
    DistributionConfig: Match.objectLike({
      DefaultCacheBehavior: Match.objectLike({
        ViewerProtocolPolicy: 'redirect-to-https',
        LambdaFunctionAssociations: [Match.objectLike({ EventType: 'origin-request' })],
      }),
    }),
  });
  template.hasResourceProperties('AWS::IAM::Policy', {
    PolicyDocument: Match.objectLike({
      Statement: Match.arrayWith([
        Match.objectLike({
          Action: 's3:GetObject',
          Resource: {
            'Fn::Join': ['', ['arn:', { Ref: 'AWS::Partition' }, ':s3:::my-wvb-bundles/*']],
          },
        }),
        Match.objectLike({
          Action: 's3:ListBucket',
          Resource: {
            'Fn::Join': ['', ['arn:', { Ref: 'AWS::Partition' }, ':s3:::my-wvb-bundles']],
          },
        }),
      ]),
    }),
  });
  expect(remote.endpoint).toBe(`https://${remote.distribution.distributionDomainName}`);
  const assetDir = readdirSync(temporary.path).find(name => name.startsWith('asset.'))!;
  const code = readFileSync(path.join(temporary.path, assetDir, 'origin-request.mjs'), 'utf8');
  expect(code.includes('"bucketName":"my-wvb-bundles","region":"us-east-1"')).toBe(true);
  expect(code.includes('__WVB_REMOTE_CONFIG__')).toBe(false);
});

it('synthesizes multiple regional remotes without edge-stack dependency cycles or name collisions', () => {
  using temporary = {
    path: mkdtempSync(path.join(tmpdir(), 'wvb-cdk-')),
    [Symbol.dispose]() {
      rmSync(this.path, { recursive: true, force: true });
    },
  };
  const app = new App({ outdir: temporary.path, autoSynth: false });
  const stack = new Stack(app, 'Remote', {
    env: { account: '123456789012', region: 'ap-northeast-2' },
  });
  new WebviewBundleRemote(stack, 'First', { bucketName: 'first-wvb-bundles' });
  new WebviewBundleRemote(stack, 'Second', { bucketName: 'second-wvb-bundles' });
  const assembly = app.synth();
  const edgeStacks = assembly.stacks.filter(
    artifact => artifact.environment.region === 'us-east-1'
  );

  expect(edgeStacks).toHaveLength(2);
  for (const artifact of edgeStacks) {
    const template = Template.fromJSON(artifact.template);
    template.resourceCountIs('AWS::Lambda::Version', 1);
    template.hasResourceProperties('AWS::Lambda::Function', { Handler: 'origin-request.handler' });
  }
  const assets = readdirSync(temporary.path)
    .filter(name => name.startsWith('asset.'))
    .filter(name => readdirSync(path.join(temporary.path, name)).includes('origin-request.mjs'));
  expect(assets).toHaveLength(2);
  const code = assets.map(name =>
    readFileSync(path.join(temporary.path, name, 'origin-request.mjs'), 'utf8')
  );
  expect(
    code.some(value => value.includes('"bucketName":"first-wvb-bundles","region":"ap-northeast-2"'))
  ).toBe(true);
  expect(
    code.some(value =>
      value.includes('"bucketName":"second-wvb-bundles","region":"ap-northeast-2"')
    )
  ).toBe(true);
});

it('rejects an unresolved bucket name before creating resources', () => {
  using temporary = {
    path: mkdtempSync(path.join(tmpdir(), 'wvb-cdk-')),
    [Symbol.dispose]() {
      rmSync(this.path, { recursive: true, force: true });
    },
  };
  const app = new App({ outdir: temporary.path, autoSynth: false });
  const stack = new Stack(app, 'Remote', { env: { account: '123456789012', region: 'us-east-1' } });
  const bucketName = new CfnParameter(stack, 'BucketName');
  expect(
    () => new WebviewBundleRemote(stack, 'Bundles', { bucketName: bucketName.valueAsString })
  ).toThrow('bucketName must be a concrete string');
  Template.fromStack(stack).resourceCountIs('AWS::S3::Bucket', 0);
});

it('requires an explicit stack region before embedding the S3 client configuration', () => {
  using temporary = {
    path: mkdtempSync(path.join(tmpdir(), 'wvb-cdk-')),
    [Symbol.dispose]() {
      rmSync(this.path, { recursive: true, force: true });
    },
  };
  const app = new App({ outdir: temporary.path, autoSynth: false });
  const stack = new Stack(app, 'Remote');
  expect(() => new WebviewBundleRemote(stack, 'Bundles', { bucketName: 'my-wvb-bundles' })).toThrow(
    'explicit stack env.region'
  );
  Template.fromStack(stack).resourceCountIs('AWS::Lambda::Function', 0);
});

it('applies resource settings and tags across the regional and edge stacks while preserving S3 access', () => {
  using temporary = {
    path: mkdtempSync(path.join(tmpdir(), 'wvb-cdk-')),
    [Symbol.dispose]() {
      rmSync(this.path, { recursive: true, force: true });
    },
  };
  const app = new App({ outdir: temporary.path, autoSynth: false });
  const stack = new Stack(app, 'Remote', {
    env: { account: '123456789012', region: 'ap-northeast-2' },
  });
  new WebviewBundleRemote(stack, 'Bundles', {
    bucketName: 'custom-wvb-bundles',
    bucketProps: {
      versioned: true,
      lifecycleRules: [{ noncurrentVersionExpiration: Duration.days(30) }],
      tags: { Resource: 'bundles' },
    },
    originRequestProps: {
      functionName: 'custom-wvb-origin-request',
      description: 'Custom remote handler',
      runtime: Runtime.NODEJS_24_X,
      timeout: Duration.seconds(15),
      memorySize: 512,
      initialPolicy: [
        new PolicyStatement({
          actions: ['kms:Sign'],
          resources: ['arn:aws:kms:us-east-1:123456789012:key/signing-key'],
        }),
      ],
      tags: { Resource: 'handler' },
    },
    distributionProps: {
      comment: 'Custom remote CDN',
      enabled: false,
      httpVersion: HttpVersion.HTTP2_AND_3,
      priceClass: PriceClass.PRICE_CLASS_100,
      tags: { Resource: 'cdn' },
    },
  });

  const assembly = app.synth();
  const regionalTemplate = Template.fromStack(stack);
  const edgeStack = assembly.stacks.find(artifact => artifact.environment.region === 'us-east-1');
  expect(edgeStack).toBeDefined();
  const edgeTemplate = Template.fromJSON(edgeStack!.template);
  regionalTemplate.hasResourceProperties('AWS::S3::Bucket', {
    VersioningConfiguration: { Status: 'Enabled' },
    LifecycleConfiguration: {
      Rules: [Match.objectLike({ NoncurrentVersionExpiration: { NoncurrentDays: 30 } })],
    },
    Tags: Match.arrayWith([{ Key: 'Resource', Value: 'bundles' }]),
  });
  regionalTemplate.hasResourceProperties('AWS::CloudFront::Distribution', {
    DistributionConfig: Match.objectLike({
      Comment: 'Custom remote CDN',
      Enabled: false,
      HttpVersion: 'http2and3',
      PriceClass: 'PriceClass_100',
      DefaultCacheBehavior: Match.objectLike({
        LambdaFunctionAssociations: [Match.objectLike({ EventType: 'origin-request' })],
      }),
    }),
    Tags: Match.arrayWith([{ Key: 'Resource', Value: 'cdn' }]),
  });
  edgeTemplate.hasResourceProperties('AWS::Lambda::Function', {
    FunctionName: 'custom-wvb-origin-request',
    Description: 'Custom remote handler',
    Runtime: 'nodejs24.x',
    Handler: 'origin-request.handler',
    Timeout: 15,
    MemorySize: 512,
    Tags: Match.arrayWith([{ Key: 'Resource', Value: 'handler' }]),
  });
  edgeTemplate.hasResourceProperties('AWS::IAM::Policy', {
    PolicyDocument: Match.objectLike({
      Statement: Match.arrayWith([
        Match.objectLike({ Action: 's3:GetObject' }),
        Match.objectLike({ Action: 's3:ListBucket' }),
        Match.objectLike({
          Action: 'kms:Sign',
          Resource: 'arn:aws:kms:us-east-1:123456789012:key/signing-key',
        }),
      ]),
    }),
  });
  edgeTemplate.hasResource('AWS::Lambda::Version', {
    DeletionPolicy: 'Retain',
    UpdateReplacePolicy: 'Retain',
  });
});

it('uses custom code and a separately managed edge role without regional dependency cycles', () => {
  using temporary = {
    path: mkdtempSync(path.join(tmpdir(), 'wvb-cdk-')),
    [Symbol.dispose]() {
      rmSync(this.path, { recursive: true, force: true });
    },
  };
  const app = new App({ outdir: temporary.path, autoSynth: false });
  const stack = new Stack(app, 'Remote', {
    env: { account: '123456789012', region: 'ap-northeast-2' },
  });
  const permissionsStack = new Stack(app, 'Permissions', {
    env: { account: '123456789012', region: 'us-east-1' },
  });
  const role = new Role(permissionsStack, 'CustomRole', {
    roleName: 'custom-wvb-role',
    assumedBy: new ServicePrincipal('lambda.amazonaws.com'),
    managedPolicies: [
      ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'),
    ],
  });
  const code = 'exports.custom = async event => event.Records[0].cf.request;';
  new WebviewBundleRemote(stack, 'Bundles', {
    bucketName: 'custom-code-wvb-bundles',
    originRequestProps: { code: Code.fromInline(code), handler: 'index.custom', role },
  });

  const assembly = app.synth();
  const edgeStack = assembly.stacks.find(artifact => artifact.stackName.startsWith('wvb-edge-'));
  expect(edgeStack).toBeDefined();
  const edgeTemplate = Template.fromJSON(edgeStack!.template);
  const permissionsTemplate = Template.fromStack(permissionsStack);
  edgeTemplate.resourceCountIs('AWS::IAM::Role', 0);
  permissionsTemplate.resourceCountIs('AWS::IAM::Role', 1);
  permissionsTemplate.hasResourceProperties('AWS::IAM::Role', {
    RoleName: 'custom-wvb-role',
    AssumeRolePolicyDocument: Match.objectLike({
      Statement: Match.arrayWith([
        Match.objectLike({ Principal: { Service: 'edgelambda.amazonaws.com' } }),
      ]),
    }),
  });
  edgeTemplate.hasResourceProperties('AWS::Lambda::Function', {
    Code: { ZipFile: code },
    Handler: 'index.custom',
    Role: { 'Fn::ImportValue': Match.anyValue() },
  });
  permissionsTemplate.hasResourceProperties('AWS::IAM::Policy', {
    Roles: [permissionsStack.resolve(role.roleName)],
    PolicyDocument: Match.objectLike({
      Statement: Match.arrayWith([
        Match.objectLike({ Action: 's3:GetObject' }),
        Match.objectLike({ Action: 's3:ListBucket' }),
      ]),
    }),
  });
  const handlerAssets = readdirSync(temporary.path)
    .filter(name => name.startsWith('asset.'))
    .filter(name => readdirSync(path.join(temporary.path, name)).includes('origin-request.mjs'));
  expect(handlerAssets).toHaveLength(0);
});
