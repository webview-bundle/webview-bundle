import fs from 'node:fs/promises';
import type { AddressInfo } from 'node:net';
import os from 'node:os';
import path from 'node:path';
import type { ServerType } from '@hono/node-server';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { Logger } from '../../log.js';
import { type LocalRemoteInstance, localRemote } from './local.js';

let baseDir: string;
let instance: LocalRemoteInstance | undefined;

beforeEach(async () => {
  baseDir = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-local-remote-'));
});

afterEach(async () => {
  await instance?.shutdown().catch(() => {});
  instance = undefined;
  await fs.rm(baseDir, { recursive: true, force: true });
});

async function startLocalRemote(params: { allowOtherVersions?: boolean; logger?: Logger }) {
  instance = await localRemote({
    baseDir,
    hostname: '127.0.0.1',
    port: 0,
    ...params,
  });
  const server: ServerType = instance.server;
  if (!server.listening) {
    await new Promise<void>(resolve => {
      server.once('listening', () => resolve());
    });
  }
  const { port } = server.address() as AddressInfo;
  return `http://127.0.0.1:${port}`;
}

function createTestLogger() {
  const messages: string[] = [];
  const record = (message: string) => {
    messages.push(message);
  };
  const logger = { debug: record, info: record, warn: record, error: record } as unknown as Logger;
  return { logger, messages };
}

describe('localRemote', () => {
  it('lists no bundles when the base dir is empty', async () => {
    const baseUrl = await startLocalRemote({});

    const res = await fetch(`${baseUrl}/bundles`);

    expect(res.status).toBe(200);
    expect(await res.json()).toEqual([]);
  });

  it('lists the deployed bundles found in the base dir', async () => {
    await fs.mkdir(path.join(baseDir, 'bundles', 'app'), { recursive: true });
    await fs.writeFile(
      path.join(baseDir, 'bundles', 'app', 'deployment.json'),
      JSON.stringify({ name: 'app', version: '1.0.0' })
    );
    const baseUrl = await startLocalRemote({});

    const res = await fetch(`${baseUrl}/bundles`);

    expect(await res.json()).toEqual([{ name: 'app', version: '1.0.0' }]);
  });

  it('returns 404 for a bundle that has no deployment', async () => {
    const baseUrl = await startLocalRemote({});

    const res = await fetch(`${baseUrl}/bundles/app`);

    expect(res.status).toBe(404);
  });

  it('rejects a version-pinned download by default', async () => {
    const baseUrl = await startLocalRemote({});

    const res = await fetch(`${baseUrl}/bundles/app/1.0.0`);

    expect(res.status).toBe(403);
  });

  it('allows a version-pinned download when allowOtherVersions is enabled', async () => {
    const baseUrl = await startLocalRemote({ allowOtherVersions: true });

    const res = await fetch(`${baseUrl}/bundles/app/1.0.0`);

    expect(res.status).not.toBe(403);
  });

  it('logs the started server address', async () => {
    const { logger, messages } = createTestLogger();

    const baseUrl = await startLocalRemote({ logger });

    const { port } = new URL(baseUrl);
    expect(messages.some(x => x.includes(`Remote started:`) && x.includes(port))).toBe(true);
  });

  it('stops serving after shutdown', async () => {
    const baseUrl = await startLocalRemote({});

    await instance!.shutdown();

    await expect(fetch(`${baseUrl}/bundles`)).rejects.toThrow();
  });
});
