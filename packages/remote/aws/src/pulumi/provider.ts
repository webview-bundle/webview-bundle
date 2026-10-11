import * as aws from '@pulumi/aws';
import * as pulumi from '@pulumi/pulumi';
import type { AssetArchive } from '@pulumi/pulumi/asset/archive.js';
import { getLambdaCode } from './lambda.js';
import type { LambdaRuntime } from './types.js';

export interface WebviewBundleRemoteLambdaCodeConfig {
  name?: pulumi.Input<string>;
  description?: pulumi.Input<string>;
  code?: pulumi.Input<AssetArchive>;
  runtime?: pulumi.Input<LambdaRuntime>;
  handler?: pulumi.Input<string>;
  timeout?: pulumi.Input<number>;
  memorySize?: pulumi.Input<number>;
  tags?: pulumi.Input<{ [key: string]: pulumi.Input<string> }>;
}

export interface WebviewBundleRemoteProviderConfig {
  name?: string;
  region?: pulumi.Input<string>;
  bucketName?: string;
  bucketForceDestroy?: pulumi.Input<boolean>;
  bucketTags?: pulumi.Input<{ [key: string]: pulumi.Input<string> }>;
  lambdaOriginRequest?: WebviewBundleRemoteLambdaCodeConfig;
  lambdaRoleName?: pulumi.Input<string>;
  lambdaPolicyActions?: string[];
  lambdaRolePolicyName?: pulumi.Input<string>;
  cloudfrontAliases?: pulumi.Input<pulumi.Input<string>[]>;
  cloudfrontComment?: pulumi.Input<string>;
  cloudfrontViewerCertificate?: aws.cloudfront.DistributionArgs['viewerCertificate'];
  cloudfrontTags?: pulumi.Input<{ [key: string]: pulumi.Input<string> }>;
  cloudfrontEnabled?: pulumi.Input<boolean>;
  cloudfrontHttpVersion?: pulumi.Input<'http1.1' | 'http2' | 'http2and3' | 'http3'>;
  cloudfrontWaitForDeployment?: pulumi.Input<boolean>;
}

export class WebviewBundleRemoteProvider extends pulumi.ComponentResource {
  public readonly bucketId: pulumi.Output<string>;
  public readonly bucketName: pulumi.Output<string>;
  public readonly bucketDomainName: pulumi.Output<string>;
  public readonly lambdaOriginRequestArn: pulumi.Output<string>;
  public readonly cloudfrontDistributionId: pulumi.Output<string>;
  public readonly cloudfrontDistributionArn: pulumi.Output<string>;
  public readonly cloudfrontDistributionDomainName: pulumi.Output<string>;

  constructor(
    name: string,
    config: WebviewBundleRemoteProviderConfig = {},
    opts?: pulumi.ComponentResourceOptions
  ) {
    super('webview-bundle:aws:RemoteProvider', name, {}, opts);

    const {
      name: baseName = 'webview-bundle',
      region,
      bucketName = `${baseName}-bucket`,
      bucketForceDestroy,
      bucketTags,
      lambdaOriginRequest,
      lambdaRoleName = `${baseName}-lambda-role`,
      lambdaPolicyActions = [],
      lambdaRolePolicyName = `${baseName}-lambda-role-policy`,
      cloudfrontAliases,
      cloudfrontEnabled = true,
      cloudfrontHttpVersion = 'http2and3',
      cloudfrontComment = `${baseName}-cdn`,
      cloudfrontViewerCertificate = {
        cloudfrontDefaultCertificate: true,
      },
      cloudfrontWaitForDeployment = true,
      cloudfrontTags,
    } = config;

    const provider = new aws.Provider(
      'provider',
      {
        region,
      },
      { parent: this }
    );
    const usEast1Provider = new aws.Provider(
      'provider_us-east-1',
      {
        region: 'us-east-1',
      },
      { parent: this }
    );

    const bucket = new aws.s3.Bucket(
      'bucket',
      {
        bucket: bucketName,
        forceDestroy: bucketForceDestroy,
        tags: bucketTags,
      },
      {
        provider,
        parent: this,
      }
    );

    const originAccessIdentity = new aws.cloudfront.OriginAccessIdentity(
      'origin_access_identity',
      {
        comment: bucketName,
      },
      { provider: usEast1Provider, parent: this }
    );
    new aws.s3.BucketPolicy(
      'bucket_policy',
      {
        bucket: bucket.id,
        policy: pulumi.all([bucket.arn, originAccessIdentity.iamArn]).apply(([bucketArn, oaiArn]) =>
          JSON.stringify({
            Version: '2012-10-17',
            Statement: [
              {
                Sid: 'AllowCloudfrontGetObject',
                Effect: 'Allow',
                Principal: {
                  AWS: oaiArn,
                },
                Action: 's3:GetObject',
                Resource: `${bucketArn}/*`,
              },
              {
                Sid: 'AllowCloudfrontListBuckets',
                Effect: 'Allow',
                Principal: {
                  AWS: oaiArn,
                },
                Action: 's3:ListBucket',
                Resource: bucketArn,
              },
            ],
          })
        ),
      },
      { provider, parent: this, dependsOn: [bucket] }
    );

    const lambdaAssumeRolePolicy = aws.iam.getPolicyDocumentOutput({
      statements: [
        {
          effect: 'Allow',
          principals: [
            {
              type: 'Service',
              identifiers: ['lambda.amazonaws.com', 'edgelambda.amazonaws.com'],
            },
          ],
          actions: ['sts:AssumeRole'],
        },
      ],
    });

    const lambdaRole = new aws.iam.Role(
      'lambda_role',
      {
        name: lambdaRoleName,
        assumeRolePolicy: lambdaAssumeRolePolicy.json,
      },
      { provider: usEast1Provider, parent: this }
    );

    const lambdaPolicy = aws.iam.getPolicyDocumentOutput({
      statements: [
        {
          effect: 'Allow',
          actions: ['s3:GetObject'],
          resources: [pulumi.interpolate`${bucket.arn}/*`],
        },
        { effect: 'Allow', actions: ['s3:ListBucket'], resources: [bucket.arn] },
        {
          effect: 'Allow',
          actions: ['logs:CreateLogGroup', 'logs:CreateLogStream', 'logs:PutLogEvents'],
          resources: ['arn:aws:logs:*:*:*'],
        },
        ...(lambdaPolicyActions.length === 0
          ? []
          : [{ effect: 'Allow', actions: lambdaPolicyActions, resources: ['*'] }]),
      ],
    });

    const lambdaRolePolicy = new aws.iam.RolePolicy(
      'lambda_role_policy',
      {
        name: lambdaRolePolicyName,
        role: lambdaRole.id,
        policy: lambdaPolicy.json,
      },
      { provider: usEast1Provider, parent: this }
    );

    const originRequestCode =
      lambdaOriginRequest?.code ??
      getLambdaCode({
        bucket: bucket.bucket,
        region: region ?? aws.getRegionOutput({}, { provider, parent: this }).region,
      });

    const lambdaOriginRequestFn = new aws.lambda.Function(
      'lambda_origin_request',
      {
        publish: true,
        tags: lambdaOriginRequest?.tags,
        name: lambdaOriginRequest?.name ?? `${baseName}-lambda-origin-request`,
        description:
          lambdaOriginRequest?.description ?? `${baseName} lambda origin request function`,
        role: lambdaRole.arn,
        runtime: lambdaOriginRequest?.runtime ?? 'nodejs22.x',
        timeout: lambdaOriginRequest?.timeout,
        memorySize: lambdaOriginRequest?.memorySize,
        code: originRequestCode,
        handler: lambdaOriginRequest?.handler ?? 'origin-request.handler',
      },
      {
        provider: usEast1Provider,
        parent: this,
        dependsOn: [bucket, lambdaRolePolicy],
      }
    );

    const cloudfrontDistribution = new aws.cloudfront.Distribution(
      'cloudfront_distribution',
      {
        aliases: cloudfrontAliases,
        enabled: cloudfrontEnabled,
        httpVersion: cloudfrontHttpVersion,
        comment: cloudfrontComment,
        tags: cloudfrontTags,
        waitForDeployment: cloudfrontWaitForDeployment,
        origins: [
          {
            domainName: bucket.bucketRegionalDomainName,
            originId: bucket.id,
            s3OriginConfig: {
              originAccessIdentity: originAccessIdentity.cloudfrontAccessIdentityPath,
            },
          },
        ],
        restrictions: {
          geoRestriction: {
            restrictionType: 'none',
          },
        },
        defaultCacheBehavior: {
          allowedMethods: ['GET', 'HEAD', 'OPTIONS'],
          cachedMethods: ['GET', 'HEAD'],
          compress: true,
          defaultTtl: 0,
          maxTtl: 31536000,
          minTtl: 0,
          targetOriginId: bucket.id,
          viewerProtocolPolicy: 'redirect-to-https',
          forwardedValues: {
            queryString: false,
            headers: [
              'wvb-update-protocol-version',
              'wvb-runtime-version',
              'wvb-update-channel',
              'wvb-expect-signature',
              'if-none-match',
            ],
            cookies: {
              forward: 'none',
            },
          },
          lambdaFunctionAssociations: [
            {
              eventType: 'origin-request',
              lambdaArn: lambdaOriginRequestFn.qualifiedArn,
            },
          ],
        },
        viewerCertificate: cloudfrontViewerCertificate,
      },
      {
        provider: usEast1Provider,
        parent: this,
      }
    );

    this.bucketId = bucket.id;
    this.bucketName = bucket.bucket;
    this.bucketDomainName = bucket.bucketDomainName;
    this.lambdaOriginRequestArn = lambdaOriginRequestFn.qualifiedArn;
    this.cloudfrontDistributionId = cloudfrontDistribution.id;
    this.cloudfrontDistributionArn = cloudfrontDistribution.arn;
    this.cloudfrontDistributionDomainName = cloudfrontDistribution.domainName;
    this.registerOutputs({
      bucketId: this.bucketId,
      bucketName: this.bucketName,
      bucketDomainName: this.bucketDomainName,
      lambdaOriginRequestArn: this.lambdaOriginRequestArn,
      cloudfrontDistributionId: this.cloudfrontDistributionId,
      cloudfrontDistributionArn: this.cloudfrontDistributionArn,
      cloudfrontDistributionDomainName: this.cloudfrontDistributionDomainName,
    });
  }
}
