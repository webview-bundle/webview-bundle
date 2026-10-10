import * as pulumi from '@pulumi/pulumi';
import { generateOriginRequestHandlerCode } from '../lambda/index.js';

interface LambdaCodeConfig {
  bucket: pulumi.Input<string>;
  region?: pulumi.Input<string>;
}

export function getLambdaCode(config: LambdaCodeConfig): pulumi.Output<pulumi.asset.AssetArchive> {
  return pulumi.all([config.bucket, config.region]).apply(async ([bucketName, region]) => {
    const code = generateOriginRequestHandlerCode({ bucketName, region });
    return new pulumi.asset.AssetArchive({
      'origin-request.mjs': new pulumi.asset.StringAsset(code),
    });
  });
}
