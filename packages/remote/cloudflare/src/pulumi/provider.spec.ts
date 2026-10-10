import * as pulumi from '@pulumi/pulumi';
import { expect, it } from 'vitest';
import { WebviewBundleRemoteProvider } from './provider.js';

it('binds R2 and KV and injects configuration into the packaged Worker', async () => {
  const resources: pulumi.runtime.MockResourceArgs[] = [];
  pulumi.runtime.setMocks(
    {
      newResource(args) {
        resources.push(args);
        return { id: args.name, state: args.inputs };
      },
      call: args => args.inputs,
    },
    'project',
    'stack',
    false
  );
  const remote = new WebviewBundleRemoteProvider('test', {
    accountId: 'account',
    workerConfig: {
      signaturePolicy: pulumi.output('strict'),
      disableIfNoneMatch: pulumi.output(true),
    },
  });
  await new Promise<void>(resolve => {
    remote.workerDeploymentId.apply(() => {
      resolve();
    });
  });
  const version = resources.find(
    resource => resource.type === 'cloudflare:index/workerVersion:WorkerVersion'
  )!;
  expect(version.inputs.compatibilityFlags).toContain('nodejs_compat');
  expect(version.inputs.bindings).toEqual([
    { type: 'r2_bucket', bucketName: 'webview-bundle', name: 'BUCKET' },
    { type: 'kv_namespace', namespaceId: 'kv', name: 'KV' },
  ]);
  expect(version.inputs.modules[0].name).toBe(version.inputs.mainModule);
  expect(version.inputs.modules[0].contentType).toBe('application/javascript+module');
  const code = Buffer.from(version.inputs.modules[0].contentBase64, 'base64').toString('utf8');
  expect(code).toContain('"signaturePolicy":"strict","disableIfNoneMatch":true');
  expect(code).not.toContain('__WVB_REMOTE_CONFIG__');
  expect(version.inputs.modules[0].contentFile).toBeUndefined();
});

import { Buffer } from 'node:buffer';

it('preserves explicitly supplied Worker modules', async () => {
  const resources: pulumi.runtime.MockResourceArgs[] = [];
  pulumi.runtime.setMocks(
    {
      newResource(args) {
        resources.push(args);
        return { id: args.name, state: args.inputs };
      },
      call: args => args.inputs,
    },
    'project',
    'stack',
    false
  );
  const modules = [
    {
      name: 'custom.mjs',
      contentType: 'application/javascript+module',
      contentBase64: Buffer.from(
        'export default { fetch() { return new Response("custom"); } };'
      ).toString('base64'),
    },
  ];
  const remote = new WebviewBundleRemoteProvider('custom', {
    accountId: 'account',
    workerConfig: { signaturePolicy: 'strict' },
    workerVersion: { mainModule: 'custom.mjs', modules },
  });
  await new Promise<void>(resolve => {
    remote.workerDeploymentId.apply(() => {
      resolve();
    });
  });
  const version = resources.find(
    resource => resource.type === 'cloudflare:index/workerVersion:WorkerVersion'
  )!;
  expect(version.inputs.mainModule).toBe('custom.mjs');
  expect(version.inputs.modules).toEqual(modules);
});
