import { createHash } from 'node:crypto';
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { AssetHashType, Duration, RemovalPolicy, Stack, Tags, Token } from 'aws-cdk-lib';
import * as cloudfront from 'aws-cdk-lib/aws-cloudfront';
import * as origins from 'aws-cdk-lib/aws-cloudfront-origins';
import * as iam from 'aws-cdk-lib/aws-iam';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as s3 from 'aws-cdk-lib/aws-s3';
import { Construct } from 'constructs';
import { getOriginRequestHandleCodeFilePath } from '../lambda/code.js';
import { generateOriginRequestHandlerCode } from '../lambda/index.js';

export interface WebviewBundleRemoteProps {
  /** A concrete, globally unique name, embedded in the Lambda bundle at synthesis time. */
  readonly bucketName: string;
  /** Storage lifecycle and CORS settings for the private, S3-encrypted bucket. */
  readonly bucketProps?: Pick<
    s3.BucketProps,
    'versioned' | 'removalPolicy' | 'autoDeleteObjects' | 'lifecycleRules' | 'cors'
  > & { readonly tags?: Readonly<Record<string, string>> };
  /** Lambda@Edge settings. Additional IAM statements are merged with the required S3 permissions. */
  readonly originRequestProps?: Partial<
    Pick<
      cloudfront.experimental.EdgeFunctionProps,
      | 'functionName'
      | 'description'
      | 'code'
      | 'runtime'
      | 'handler'
      | 'timeout'
      | 'memorySize'
      | 'role'
      | 'initialPolicy'
    >
  > & { readonly tags?: Readonly<Record<string, string>> };
  /** Distribution settings; the remote controls its default behavior and origin-request handler. */
  readonly distributionProps?: Omit<cloudfront.DistributionProps, 'defaultBehavior'> & {
    readonly tags?: Readonly<Record<string, string>>;
  };
}

/** S3 remote with a CloudFront distribution and a published Lambda@Edge handler. */
export class WebviewBundleRemote extends Construct {
  readonly bucket: s3.Bucket;
  readonly originRequest: cloudfront.experimental.EdgeFunction;
  readonly distribution: cloudfront.Distribution;
  readonly endpoint: string;

  constructor(scope: Construct, id: string, props: WebviewBundleRemoteProps) {
    super(scope, id);
    const stack = Stack.of(this);
    if (Token.isUnresolved(props.bucketName)) {
      throw new Error(
        'bucketName must be a concrete string because it is embedded in the Lambda@Edge code.'
      );
    }
    if (Token.isUnresolved(stack.region)) {
      throw new Error('WebviewBundleRemote requires an explicit stack env.region.');
    }

    const { tags: bucketTags, ...bucketProps } = props.bucketProps ?? {};
    const { tags: originRequestTags, ...originRequestProps } = props.originRequestProps ?? {};
    const { tags: distributionTags, ...distributionProps } = props.distributionProps ?? {};

    this.bucket = new s3.Bucket(this, 'Bucket', {
      blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL,
      encryption: s3.BucketEncryption.S3_MANAGED,
      enforceSSL: true,
      ...bucketProps,
      bucketName: props.bucketName,
    });
    for (const [key, value] of Object.entries(bucketTags ?? {})) {
      Tags.of(this.bucket).add(key, value);
    }
    // A literal bucket ARN avoids a dependency cycle between the regional stack and edge stack.
    const bucketArn = stack.formatArn({
      service: 's3',
      region: '',
      account: '',
      resource: props.bucketName,
    });
    this.originRequest = new cloudfront.experimental.EdgeFunction(this, 'OriginRequest', {
      stackId: `wvb-edge-${this.node.addr}`,
      functionName: originRequestProps.functionName,
      description: originRequestProps.description,
      runtime: originRequestProps.runtime ?? lambda.Runtime.NODEJS_22_X,
      handler: originRequestProps.handler ?? 'origin-request.handler',
      timeout: originRequestProps.timeout ?? Duration.seconds(30),
      memorySize: originRequestProps.memorySize ?? 128,
      role: originRequestProps.role,
      // Edge replicas outlive distribution updates; immediate version deletion can fail.
      currentVersionOptions: { removalPolicy: RemovalPolicy.RETAIN },
      code:
        originRequestProps.code ?? this.#createOriginRequestCode(props.bucketName, stack.region),
      initialPolicy: [
        new iam.PolicyStatement({ actions: ['s3:GetObject'], resources: [`${bucketArn}/*`] }),
        new iam.PolicyStatement({ actions: ['s3:ListBucket'], resources: [bucketArn] }),
        ...(originRequestProps.initialPolicy ?? []),
      ],
    });
    // Tag the actual function, which can live in a separate us-east-1 stack.
    for (const [key, value] of Object.entries(originRequestTags ?? {})) {
      Tags.of(this.originRequest.lambda).add(key, value);
    }

    const cachePolicy = new cloudfront.CachePolicy(this, 'CachePolicy', {
      minTtl: Duration.seconds(0),
      defaultTtl: Duration.seconds(0),
      maxTtl: Duration.days(365),
      cookieBehavior: cloudfront.CacheCookieBehavior.none(),
      queryStringBehavior: cloudfront.CacheQueryStringBehavior.none(),
      headerBehavior: cloudfront.CacheHeaderBehavior.allowList(
        'wvb-update-protocol-version',
        'wvb-runtime-version',
        'wvb-update-channel',
        'wvb-expect-signature',
        'if-none-match'
      ),
      enableAcceptEncodingGzip: true,
      enableAcceptEncodingBrotli: true,
    });
    this.distribution = new cloudfront.Distribution(this, 'Distribution', {
      ...distributionProps,
      defaultBehavior: {
        origin: origins.S3BucketOrigin.withOriginAccessControl(this.bucket),
        viewerProtocolPolicy: cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
        allowedMethods: cloudfront.AllowedMethods.ALLOW_GET_HEAD_OPTIONS,
        cachedMethods: cloudfront.CachedMethods.CACHE_GET_HEAD,
        compress: true,
        cachePolicy,
        edgeLambdas: [
          {
            functionVersion: this.originRequest.currentVersion,
            eventType: cloudfront.LambdaEdgeEventType.ORIGIN_REQUEST,
          },
        ],
      },
    });
    for (const [key, value] of Object.entries(distributionTags ?? {})) {
      Tags.of(this.distribution).add(key, value);
    }
    this.endpoint = `https://${this.distribution.distributionDomainName}`;
  }

  #createOriginRequestCode(bucketName: string, region: string): lambda.Code {
    const code = generateOriginRequestHandlerCode({ bucketName, region });

    return lambda.Code.fromAsset(path.dirname(getOriginRequestHandleCodeFilePath()), {
      assetHashType: AssetHashType.CUSTOM,
      assetHash: createHash('sha256').update(code).digest('hex'),
      bundling: {
        image: lambda.Runtime.NODEJS_22_X.bundlingImage,
        local: {
          tryBundle(outputDir) {
            writeFileSync(path.join(outputDir, 'origin-request.mjs'), code);
            return true;
          },
        },
      },
    });
  }
}
