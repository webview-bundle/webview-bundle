import path from 'node:path';
import { fileURLToPath } from 'node:url';
import * as pulumi from '@pulumi/pulumi';
import type { WebviewBundleRemoteConfig } from '@wvb/remote-aws-provider';
import { generateCode } from './code.js';
import { getLambdaRuntimeTarget, type LambdaRuntime } from './types.js';

const dirname =
  typeof import.meta.dirname === 'string'
    ? import.meta.dirname
    : path.dirname(fileURLToPath(import.meta.url));

export interface LambdaCodeConfig {
  bucket: pulumi.Output<string>;
  region?: pulumi.Input<string>;
  runtime?: pulumi.Input<LambdaRuntime>;
  esm?: pulumi.Input<boolean>;
  sourcemap?: pulumi.Input<boolean>;
  minify?: pulumi.Input<boolean>;
}

export function getLambdaCode(
  filename: string,
  config: LambdaCodeConfig,
  allowOtherVersions?: boolean
): pulumi.Input<pulumi.asset.AssetArchive> {
  return pulumi
    .all([
      config.bucket,
      config.region,
      config.runtime ?? 'nodejs22.x',
      config.esm ?? true,
      config.sourcemap ?? true,
      config.minify ?? true,
    ])
    .apply(async ([bucketName, region, runtime, esm, sourcemap, minify]) => {
      const config: WebviewBundleRemoteConfig = {
        bucketName,
        region,
        allowOtherVersions,
      };
      const input = path.join(dirname, '..', 'lambda', filename);
      const codes = await generateCode(input, {
        platform: 'node',
        target: getLambdaRuntimeTarget(runtime),
        format: esm ? 'esm' : 'cjs',
        sourcemap,
        minify,
        define: {
          __CONFIG__: JSON.stringify(config),
        },
      });
      const assets = Object.fromEntries(
        codes.map(code => {
          return [code.fileName, new pulumi.asset.StringAsset(code.content)];
        })
      );
      return new pulumi.asset.AssetArchive(assets);
    });
}
