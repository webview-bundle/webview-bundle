import type { ServerType } from '@hono/node-server';
import { readBundle, type UriPathResolver } from '@wvb/node';
import { c, isColorEnabled } from '../console.js';
import { pathExists, toAbsolutePath, withWvbExtension } from '../fs.js';
import type { Logger } from '../log.js';
import { ApiError } from './error.js';

export interface ServeParams {
  file: string;
  hostname?: string;
  port?: number;
  silent?: boolean;
  pathResolver?: UriPathResolver;
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
    hostname,
    port = 4312,
    pathResolver = 'directory_index',
    cwd = process.cwd(),
    silent = false,
    logger,
    colorEnabled = isColorEnabled(),
  } = params;
  const filepath = toAbsolutePath(withWvbExtension(file), cwd);

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
    const p = resolvePath(c.req.path, pathResolver);
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
  const server = serve({ fetch: app.fetch, hostname, port }, info => {
    logger?.info(`Server started: ${c.success(`http://${info.address}:${info.port}`)}`);
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

function resolvePath(path: string, resolver: UriPathResolver): string {
  const decoded = decodePath(path);
  switch (resolver) {
    case 'exact':
      return decoded;
    case 'directory_index': {
      const p = decoded === '' ? '/' : decoded;
      if (p.endsWith('/')) {
        return `${p}index.html`;
      }
      return isExtensionless(p) ? `${p}/index.html` : p;
    }
    case 'html_extension': {
      if (decoded === '' || decoded === '/') {
        return '/index.html';
      }
      const p = decoded.endsWith('/') ? decoded.slice(0, -1) : decoded;
      return isExtensionless(p) ? `${p}.html` : p;
    }
  }
}

function decodePath(path: string): string {
  try {
    return decodeURIComponent(path);
  } catch {
    return path;
  }
}

function isExtensionless(path: string): boolean {
  const lastSegment = path.slice(path.lastIndexOf('/') + 1);
  return lastSegment.length > 0 && !lastSegment.includes('.');
}
