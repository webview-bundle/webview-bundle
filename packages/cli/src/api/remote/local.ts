import type { WebviewBundleServerInstance } from '@wvb/remote-local/server';
import { c } from '../../console.js';
import type { Logger } from '../../log.js';

export interface LocalRemoteParams {
  baseDir?: string;
  hostname?: string;
  port?: number;
  silent?: boolean;
  logger?: Logger;
  colorEnabled?: boolean;
}

export type LocalRemoteInstance = WebviewBundleServerInstance;

export async function localRemote(params: LocalRemoteParams): Promise<LocalRemoteInstance> {
  const { baseDir, hostname, port = 4313, logger } = params;

  const { webviewBundleServer } = await import('@wvb/remote-local/server');

  const server = webviewBundleServer({ baseDir });
  const instance = server.serve({
    hostname,
    port,
    onListen: info => {
      logger?.info(`Remote started: ${c.success(`http://${info.address}:${info.port}`)}`);
    },
  });

  return instance;
}
