import { expect, test } from 'vitest';

//#region testing/methods.ts
const METHOD_SPECS = [
  {
    id: 'source.listBundles',
    namespace: 'source',
    method: 'listBundles',
    params: [],
    summary: 'List builtin and remote bundle versions.',
  },
  {
    id: 'source.listBuiltinBundles',
    namespace: 'source',
    method: 'listBuiltinBundles',
    params: [],
    summary: 'List bundled application assets.',
  },
  {
    id: 'source.listRemoteBundles',
    namespace: 'source',
    method: 'listRemoteBundles',
    params: [],
    summary: 'List downloaded bundle versions.',
  },
  {
    id: 'source.getVersion',
    namespace: 'source',
    method: 'getVersion',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
    ],
    summary: 'Resolve the active bundle version and source.',
  },
  {
    id: 'source.getRemoteStagedVersion',
    namespace: 'source',
    method: 'getRemoteStagedVersion',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
    ],
    summary: 'Read the downloaded version waiting for installation.',
  },
  {
    id: 'source.getRemotePreviousVersion',
    namespace: 'source',
    method: 'getRemotePreviousVersion',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
    ],
    summary: 'Read the version available for rollback.',
  },
  {
    id: 'source.getBuiltinVersionData',
    namespace: 'source',
    method: 'getBuiltinVersionData',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
      {
        name: 'version',
        kind: 'string',
        defaultValue: '0.4.0',
      },
    ],
    summary: 'Read builtin integrity and metadata.',
  },
  {
    id: 'source.getRemoteVersionData',
    namespace: 'source',
    method: 'getRemoteVersionData',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
      {
        name: 'version',
        kind: 'string',
        defaultValue: '0.4.0',
      },
    ],
    summary: 'Read downloaded integrity and metadata.',
  },
  {
    id: 'source.updateRemoteVersion',
    namespace: 'source',
    method: 'updateRemoteVersion',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
      {
        name: 'version',
        kind: 'string',
        defaultValue: '0.4.0',
      },
    ],
    summary: 'Activate a version recorded in the remote manifest.',
  },
  {
    id: 'source.updateRemoteVersions',
    namespace: 'source',
    method: 'updateRemoteVersions',
    params: [
      {
        name: 'items',
        kind: 'json',
        defaultValue: '{}',
      },
    ],
    summary: 'Activate multiple remote versions.',
  },
  {
    id: 'source.stageRemoteBundle',
    namespace: 'source',
    method: 'stageRemoteBundle',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed-stage',
      },
      {
        name: 'data',
        kind: 'json',
        defaultValue: '{"version":"1"}',
      },
    ],
    summary: 'Record a downloaded version as staged.',
  },
  {
    id: 'source.stageRemoteBundles',
    namespace: 'source',
    method: 'stageRemoteBundles',
    params: [
      {
        name: 'items',
        kind: 'json',
        defaultValue: '{}',
      },
    ],
    summary: 'Stage multiple versions and their metadata.',
  },
  {
    id: 'source.removeRemoteBundle',
    namespace: 'source',
    method: 'removeRemoteBundle',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'missing',
      },
      {
        name: 'version',
        kind: 'string',
        defaultValue: '0.4.0',
      },
      {
        name: 'force',
        kind: 'json',
        defaultValue: 'false',
      },
    ],
    summary: 'Remove a remote version; current versions require force.',
  },
  {
    id: 'source.removeRemoteBundles',
    namespace: 'source',
    method: 'removeRemoteBundles',
    params: [
      {
        name: 'items',
        kind: 'json',
        defaultValue: '{}',
      },
    ],
    summary: 'Remove versions from multiple bundles.',
  },
  {
    id: 'source.pruneRemoteBundle',
    namespace: 'source',
    method: 'pruneRemoteBundle',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'missing',
      },
    ],
    summary: 'Remove orphan versions of a bundle.',
  },
  {
    id: 'source.pruneRemoteBundles',
    namespace: 'source',
    method: 'pruneRemoteBundles',
    params: [
      {
        name: 'bundleNames',
        kind: 'json',
        defaultValue: '[]',
      },
    ],
    summary: 'Remove orphan versions of multiple bundles.',
  },
  {
    id: 'source.resolveFilepath',
    namespace: 'source',
    method: 'resolveFilepath',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
    ],
    summary: 'Resolve the active bundle file.',
  },
  {
    id: 'source.getBuiltinBundleFilepath',
    namespace: 'source',
    method: 'getBuiltinBundleFilepath',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
      {
        name: 'version',
        kind: 'string',
        defaultValue: '0.4.0',
      },
    ],
    summary: 'Resolve a builtin bundle file.',
  },
  {
    id: 'source.getRemoteBundleFilepath',
    namespace: 'source',
    method: 'getRemoteBundleFilepath',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
      {
        name: 'version',
        kind: 'string',
        defaultValue: '0.4.0',
      },
    ],
    summary: 'Resolve a remote bundle file.',
  },
  {
    id: 'source.unload',
    namespace: 'source',
    method: 'unload',
    params: [
      {
        name: 'bundleName',
        kind: 'string',
        defaultValue: 'testbed',
      },
    ],
    summary: 'Drop the cached descriptor.',
  },
  {
    id: 'remote.getUpdate',
    namespace: 'remote',
    method: 'getUpdate',
    params: [
      {
        name: 'options',
        kind: 'json',
        defaultValue: '',
      },
    ],
    summary: 'Fetch an update document, ETag and signature; unchanged responses are null.',
  },
  {
    id: 'remote.download',
    namespace: 'remote',
    method: 'download',
    params: [
      {
        name: 'url',
        kind: 'string',
        defaultValue: 'http://127.0.0.1:1/missing',
      },
      {
        name: 'filepath',
        kind: 'string',
        defaultValue: '',
      },
    ],
    summary: 'Download a URL to a file without staging it.',
  },
  {
    id: 'updater.getUpdate',
    namespace: 'updater',
    method: 'getUpdate',
    params: [
      {
        name: 'options',
        kind: 'json',
        defaultValue: '',
      },
    ],
    summary: 'Find bundle updates, optionally requiring a signature key ID.',
  },
  {
    id: 'updater.download',
    namespace: 'updater',
    method: 'download',
    params: [
      {
        name: 'bundleUpdates',
        kind: 'json',
        defaultValue: '[]',
      },
      {
        name: 'options',
        kind: 'json',
        defaultValue: '',
      },
    ],
    summary: 'Download and stage bundle updates; inspect every result.type.',
  },
  {
    id: 'updater.install',
    namespace: 'updater',
    method: 'install',
    params: [
      {
        name: 'targets',
        kind: 'json',
        defaultValue: '[]',
      },
    ],
    summary: 'Install staged versions; inspect every result.type.',
  },
  {
    id: 'updater.rollback',
    namespace: 'updater',
    method: 'rollback',
    params: [
      {
        name: 'targets',
        kind: 'json',
        defaultValue: '[]',
      },
    ],
    summary: 'Restore previous versions; inspect every result.type.',
  },
];
//#endregion
//#region testing/selectors.ts
const TESTID = {
  appShell: 'app-shell',
  platformType: 'platform-type',
  version: 'version',
  invokeName: 'invoke-name',
  invokeParams: 'invoke-params',
  invokeRun: 'run-invoke',
  invokeResult: 'result-invoke',
  invokeStatus: 'status-invoke',
};
const byTestId = id => `[data-testid="${id}"]`;
const tid = {
  method: id => `method-${id}`,
  param: (id, name) => `param-${id}-${name}`,
  run: id => `run-${id}`,
  result: id => `result-${id}`,
  status: id => `status-${id}`,
};
const sel = Object.fromEntries(Object.entries(TESTID).map(([key, id]) => [key, byTestId(id)]));
const methodSel = {
  method: id => byTestId(tid.method(id)),
  param: (id, name) => byTestId(tid.param(id, name)),
  run: id => byTestId(tid.run(id)),
  result: id => byTestId(tid.result(id)),
  status: id => byTestId(tid.status(id)),
};
//#endregion
//#region testing/run-method.ts
async function runMethod(driver, id, params = {}) {
  await driver.goto('/');
  await driver.waitForVisible(sel.appShell);
  for (const [key, value] of Object.entries(params))
    await driver.fill(methodSel.param(id, key), value);
  await driver.click(methodSel.run(id));
  await driver.waitForVisible(methodSel.result(id), { timeoutMs: 9e4 });
  return {
    status: await driver.getAttribute(methodSel.result(id), 'data-status'),
    value: JSON.parse(await driver.text(methodSel.result(id))),
  };
}
//#endregion
//#region testing/update-suite.ts
function defineTestbedUpdateSuite(getDriver, getRemote) {
  async function run(id, params = {}) {
    const outcome = await runMethod(getDriver(), id, params);
    expect(outcome.status, `${id}: ${JSON.stringify(outcome.value)}`).toBe('ok');
    return outcome.value;
  }
  test('source UI shows nested manifest items and nullable versions', async () => {
    expect(await run('source.listBuiltinBundles')).toContainEqual({
      source: 'builtin',
      item: expect.objectContaining({
        name: 'testbed',
        version: '0.4.0',
        status: 'current',
        data: expect.any(Object),
      }),
    });
    expect(await run('source.getVersion', { bundleName: 'testbed' })).toEqual({
      source: 'builtin',
      version: '0.4.0',
    });
    expect(await run('source.getVersion', { bundleName: 'missing' })).toBeNull();
  });
  test('remote UI fetches an update and handles an unchanged ETag', async () => {
    const response = await run('remote.getUpdate');
    expect(response.update.bundles).toContainEqual(
      expect.objectContaining({
        name: 'testbed',
        version: '0.4.1',
      })
    );
    expect(
      await run('remote.getUpdate', { options: JSON.stringify({ etag: response.etag }) })
    ).toBeNull();
  });
  test('UI downloads, installs, reloads the new page and rolls back', async () => {
    const driver = getDriver();
    try {
      for (const version of ['0.4.1', '0.4.2']) {
        getRemote().setVersion(version);
        const update = await run('updater.getUpdate');
        expect(update.bundles).toContainEqual(
          expect.objectContaining({
            name: 'testbed',
            version,
          })
        );
        const downloads = await run('updater.download', {
          bundleUpdates: JSON.stringify([
            ...update.bundles,
            {
              name: 'missing-download',
              version,
              downloadUrl: `${getRemote().endpoint}/missing`,
            },
          ]),
          options: JSON.stringify({
            concurrency: 2,
            timeout: 1e4,
          }),
        });
        expect(downloads.find(item => item.name === 'testbed')?.result).toEqual({
          type: 'downloaded',
        });
        expect(downloads.find(item => item.name === 'missing-download')?.result).toMatchObject({
          type: 'error',
          code: expect.stringMatching(/^core\./),
        });
        expect(await run('source.getRemoteStagedVersion', { bundleName: 'testbed' })).toBe(version);
        expect(
          await run('updater.install', {
            targets: JSON.stringify([
              {
                name: 'testbed',
                version,
              },
            ]),
          })
        ).toEqual([
          {
            name: 'testbed',
            targetVersion: version,
            installVersion: version,
            result: { type: 'installed' },
          },
        ]);
        await driver.goto(`/?installed=${version}`);
        await driver.waitForVisible(sel.version);
        expect(await driver.text(sel.version)).toBe(version);
        expect(await run('updater.getUpdate')).toBeNull();
      }
      expect(
        await run('updater.rollback', { targets: JSON.stringify([{ name: 'testbed' }]) })
      ).toEqual([
        {
          name: 'testbed',
          rollbackVersion: '0.4.1',
          result: { type: 'rolled_back' },
        },
      ]);
      await driver.goto('/?rollback');
      await driver.waitForVisible(sel.version);
      expect(await driver.text(sel.version)).toBe('0.4.1');
      expect(await run('source.getVersion', { bundleName: 'testbed' })).toEqual({
        source: 'remote',
        version: '0.4.1',
      });
    } finally {
      await run('source.removeRemoteBundles', {
        items: JSON.stringify({
          testbed: {
            versions: ['0.4.1', '0.4.2'],
            force: true,
          },
        }),
      });
      getRemote().setVersion('0.4.1');
    }
  }, 3e5);
  test('manifest UI handles batch staging, metadata and guarded removal', async () => {
    const name = 'ui-stage';
    try {
      expect(
        await run('source.stageRemoteBundles', {
          items: JSON.stringify({
            [name]: {
              version: '1',
              data: { metadata: { track: 'beta' } },
            },
          }),
        })
      ).toEqual([
        {
          name,
          version: '1',
          kind: 'staged',
        },
      ]);
      expect(
        await run('source.getRemoteVersionData', {
          bundleName: name,
          version: '1',
        })
      ).toEqual({ metadata: { track: 'beta' } });
      expect(
        await run('source.updateRemoteVersions', { items: JSON.stringify({ [name]: '1' }) })
      ).toEqual([
        {
          name,
          version: '1',
          kind: 'settled',
        },
      ]);
      expect(
        await run('source.removeRemoteBundle', {
          bundleName: name,
          version: '1',
        })
      ).toEqual({
        name,
        version: '1',
        kind: 'in_use',
      });
      expect(
        await run('source.pruneRemoteBundles', { bundleNames: JSON.stringify([name]) })
      ).toEqual([
        {
          name,
          prunedVersions: [],
        },
      ]);
    } finally {
      await run('source.removeRemoteBundles', {
        items: JSON.stringify({
          [name]: {
            versions: ['1'],
            force: true,
          },
        }),
      });
    }
  });
  test('invalid download options render a native error in the UI', async () => {
    const outcome = await runMethod(getDriver(), 'updater.download', {
      options: '{"concurrency":-1}',
    });
    expect(outcome.status).toBe('error');
    expect(outcome.value).toMatchObject({
      code: 'invalid_params',
      message: expect.any(String),
    });
  });
}
//#endregion
//#region testing/index.ts
const testCases = [
  {
    name: 'detects and displays the platform',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.platformType);
      expect(await driver.text(sel.platformType)).not.toBe('none');
    },
  },
  ...METHOD_SPECS.map(spec => ({
    name: `${spec.id} invokes and renders an outcome`,
    run: async driver => {
      const outcome = await runMethod(driver, spec.id);
      expect(outcome.status).toBeOneOf(['ok', 'error']);
      if (outcome.status === 'error') {
        expect(outcome.value).toHaveProperty('message');
        expect(outcome.value).toHaveProperty('code', expect.any(String));
        expect(outcome.value).not.toMatchObject({ code: 'handler_not_found' });
        expect(outcome.value).not.toMatchObject({ code: 'unknown_platform' });
      }
    },
  })),
  {
    name: 'raw invoke calls a command through the UI',
    run: async driver => {
      await driver.goto('/');
      await driver.waitForVisible(sel.appShell);
      await driver.fill(sel.invokeName, 'sourceListBundles');
      await driver.click(sel.invokeRun);
      await driver.waitForVisible(sel.invokeResult);
      expect(await driver.getAttribute(sel.invokeResult, 'data-status')).toBe('ok');
      expect(JSON.parse(await driver.text(sel.invokeResult))).toEqual(expect.any(Array));
    },
  },
];
function defineTestbedSuite(getDriver) {
  for (const testCase of testCases) test(testCase.name, async () => testCase.run(getDriver()));
}

//#endregion
export {
  byTestId,
  defineTestbedSuite,
  defineTestbedUpdateSuite,
  METHOD_SPECS,
  methodSel,
  runMethod,
  sel,
  TESTID,
  testCases,
  tid,
};
