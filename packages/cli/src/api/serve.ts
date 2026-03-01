import type { ServerType } from '@hono/node-server';
import { readBundle } from '@wvb/node';
import type { Logger } from '../log.js';
import { c, isColorEnabled } from '../console.js';
import { pathExists, toAbsolutePath, withWVBExtension } from '../fs.js';
import { ApiError } from './error.js';

export interface ServeParams {
  file: string;
  port?: number;
  silent?: boolean;
  cwd?: string;
  logger?: Logger;
  colorEnabled?: boolean;
}

export interface ServeInstance {
  server: ServerType;
  shutdown(): Promise<void>;
}

/**
 * Serve Webview Bundle files with localhost server.
 */
export async function serve(params: ServeParams): Promise<ServeInstance> {
  const {
    file,
    port = 4312,
    cwd,
    silent = false,
    logger,
    colorEnabled = isColorEnabled(),
  } = params;
  const filepath = toAbsolutePath(withWVBExtension(file), cwd);

  if (!(await pathExists(filepath))) {
    const message = `File does not exist: ${filepath}`;
    logger?.error(message);
    throw new ApiError(message);
  }

  const { Hono } = await import('hono');
  const { serve } = await import('@hono/node-server');

  const bundle = await readBundle(filepath);
  const app = new Hono();
  if (!silent) {
    const { logMiddleware } = await import('../utils/hono-logger.js');
    app.use(
      logMiddleware(str => {
        logger?.info(str);
      }, colorEnabled)
    );
  }
  app.get('*', async c => {
    const p = resolvePath(c.req.path);
    if (!bundle.descriptor().index().containsPath(p)) {
      return c.notFound();
    }
    const entry = bundle.descriptor().index().getEntry(p)!;
    logger?.debug(
      `Read entry: ${p} (content-type=${entry.contentType}, content-length=${entry.contentLength})`
    );
    const data = bundle.getData(p)!;
    for (const [name, value] of Object.entries(entry.headers)) {
      c.header(name, value, { append: true });
    }
    c.header('content-type', entry.contentType);
    c.header('content-length', String(entry.contentLength));
    return c.body(data as Uint8Array<ArrayBuffer>, 200);
  });
  const server = serve({ fetch: app.fetch, port }, info => {
    logger?.info(`Server started: ${c.success(`http://localhost:${info.port}`)}`);
  });
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
  const instance: ServeInstance = {
    server,
    shutdown,
  };
  return instance;
}

function resolvePath(path: string): string {
  if (path.endsWith('/')) {
    return `${path}index.html`;
  }
  const ext = path.split('.').pop();
  if (ext == null && !path.includes('.')) {
    return `${path}/index.html`;
  }
  return path;
}
