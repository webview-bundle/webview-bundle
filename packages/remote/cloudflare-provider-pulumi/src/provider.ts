import path from 'node:path';
import { fileURLToPath } from 'node:url';
import type { WorkerArgs, WorkersDeploymentArgs } from '@pulumi/cloudflare';
import * as cloudflare from '@pulumi/cloudflare';
import type { R2BucketArgs } from '@pulumi/cloudflare/r2bucket.js';
import type { WorkersKvNamespaceArgs } from '@pulumi/cloudflare/workersKvNamespace.js';
import type { WorkerVersionArgs } from '@pulumi/cloudflare/workerVersion.js';
import * as pulumi from '@pulumi/pulumi';
import type { Optional } from './types.js';

const dirname =
  typeof import.meta.dirname === 'string'
    ? import.meta.dirname
    : path.dirname(fileURLToPath(import.meta.url));

export interface WebviewBundleRemoteProviderConfig {
  accountId: pulumi.Input<string>;
  bucket?: Optional<R2BucketArgs, 'accountId' | 'name'>;
  kv?: Optional<WorkersKvNamespaceArgs, 'accountId' | 'title'>;
  worker?: Optional<WorkerArgs, 'accountId' | 'name'>;
  workerVersion?: Optional<Omit<WorkerVersionArgs, 'workerId'>, 'accountId'>;
  workerDeploymentPercentage?: pulumi.Input<number>;
  workerDeploymentAnnotations?: WorkersDeploymentArgs['annotations'];
}

export class WebviewBundleRemoteProvider extends pulumi.ComponentResource {
  public readonly bucketName: pulumi.Output<string>;
  public readonly kvNamespaceId: pulumi.Output<string>;
  public readonly workerId: pulumi.Output<string>;
  public readonly workerVersionId: pulumi.Output<string>;
  public readonly workerDeploymentId: pulumi.Output<string>;

  constructor(
    name: string,
    config: WebviewBundleRemoteProviderConfig,
    opts?: pulumi.ComponentResourceOptions
  ) {
    super('webview-bundle:cloudflare:RemoteProvider', name, {}, opts);

    const {
      accountId,
      bucket: bucketArgs,
      kv: kvArgs,
      worker: workerArgs,
      workerVersion: workerVersionArgs,
      workerDeploymentPercentage = 100,
      workerDeploymentAnnotations,
    } = config;

    const bucket = new cloudflare.R2Bucket(
      'bucket',
      {
        accountId,
        name: 'webview-bundle',
        ...bucketArgs,
      },
      { parent: this }
    );
    const bucketName = pulumi.output(bucket).apply(x => x.name);

    const kv = new cloudflare.WorkersKvNamespace(
      'kv',
      {
        accountId,
        title: 'webview-bundle',
        ...kvArgs,
      },
      { parent: this }
    );

    const worker = new cloudflare.Worker(
      'worker',
      {
        accountId,
        name: 'webview-bundle',
        ...workerArgs,
      },
      { parent: this, dependsOn: [bucket, kv] }
    );
    const workerName = pulumi.output(worker).apply(x => x.name);

    const defaultScriptName = workerName.apply(x => `${x}.mjs`);
    const defaultWorkerFile = path.join(dirname, 'worker-script.js');
    const defaultWorkerSourcemapFile = path.join(dirname, 'worker-script.js.map');

    const workerMainModule =
      workerVersionArgs?.mainModule == null ? defaultScriptName : workerVersionArgs?.mainModule;
    const workerModules =
      workerVersionArgs?.modules == null
        ? [
            {
              name: workerMainModule,
              contentType: 'application/javascript+module',
              contentFile: defaultWorkerFile,
            },
            {
              name: pulumi.output(workerMainModule).apply(x => `${x}.map`),
              contentType: 'application/source-map',
              contentFile: defaultWorkerSourcemapFile,
            },
          ]
        : workerVersionArgs.modules;
    const workerBindings =
      workerVersionArgs?.bindings == null
        ? [
            {
              type: 'r2_bucket',
              bucketName: bucketName,
              name: 'BUCKET',
            },
            {
              type: 'kv_namespace',
              namespaceId: kv.id,
              name: 'KV',
            },
          ]
        : workerVersionArgs.bindings;

    const workerVersion = new cloudflare.WorkerVersion(
      'worker_version',
      {
        workerId: worker.id,
        accountId,
        ...workerVersionArgs,
        mainModule: workerMainModule,
        bindings: workerBindings,
        modules: workerModules,
      },
      { parent: this, dependsOn: [worker] }
    );

    const workerDeployment = new cloudflare.WorkersDeployment(
      'worker_deployment',
      {
        accountId,
        scriptName: workerName,
        strategy: 'percentage',
        annotations: workerDeploymentAnnotations,
        versions: [
          {
            percentage: workerDeploymentPercentage,
            versionId: workerVersion.id,
          },
        ],
      },
      { parent: this, dependsOn: [worker, workerVersion] }
    );

    this.bucketName = bucket.name;
    this.kvNamespaceId = kv.id;
    this.workerId = worker.id;
    this.workerVersionId = workerVersion.id;
    this.workerDeploymentId = workerDeployment.id;
    this.registerOutputs({
      bucketName: this.bucketName,
      kvNamespaceId: this.kvNamespaceId,
      workerId: this.workerId,
      workerVersionId: this.workerVersionId,
      workerDeploymentId: this.workerDeploymentId,
    });
  }
}

export const WvbRemoteProvider = WebviewBundleRemoteProvider;
