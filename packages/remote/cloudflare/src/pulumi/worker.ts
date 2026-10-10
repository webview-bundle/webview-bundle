import fs from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import * as pulumi from '@pulumi/pulumi';
import type { WorkerConfig } from '../worker/worker.js';

/** JSON configuration embedded in the packaged Worker. */
export interface WorkerScriptConfig {
  signaturePolicy?: pulumi.Input<WorkerConfig['signaturePolicy']>;
  disableIfNoneMatch?: pulumi.Input<boolean>;
}

export function getWorkerScript(config: WorkerScriptConfig = {}): pulumi.Output<string> {
  return pulumi
    .all([config.signaturePolicy, config.disableIfNoneMatch])
    .apply(async ([signaturePolicy, disableIfNoneMatch]) => {
      const code = await fs.readFile(getWorkerScriptPath(), 'utf8');
      return code.replaceAll('__WVB_REMOTE_CONFIG__', () =>
        JSON.stringify({ signaturePolicy, disableIfNoneMatch })
      );
    });
}

function getWorkerScriptPath(): string {
  const require = createRequire(import.meta.url);
  const packagePath = path.dirname(require.resolve('@wvb/remote-cloudflare/package.json'));
  return path.join(packagePath, 'dist', 'assets', 'worker.mjs');
}
