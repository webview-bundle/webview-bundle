import type { ServerType } from '@hono/node-server';
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

export interface LocalRemoteInstance {
  server: ServerType;
  shutdown(): Promise<void>;
}

export async function localRemote(params: LocalRemoteParams): Promise<LocalRemoteInstance> {
  const { baseDir, hostname, port = 4313, logger } = params;

  const { buildWvbServer } = await import('@wvb/remote-local/server');
  const { serve } = await import('@hono/node-server');

  const app = buildWvbServer({ baseDir });
  const server = serve(
    {
      fetch: app.fetch,
      hostname,
      port,
    },
    info => {
      logger?.info(`Remote started: ${c.success(`http://${info.address}:${info.port}`)}`);
    }
  );
  const shutdown = () => {
    return new Promise<void>((resolve, reject) => {
      server.close(error => {
        if (error != null) {
          logger?.error(`Server shutdown failed: {error}`, { error });
          reject(error);
        } else {
          resolve();
        }
      });
    });
  };
  const instance: LocalRemoteInstance = {
    server,
    shutdown,
  };
  return instance;
}
