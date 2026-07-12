import {
  BundleProtocol,
  type BundleSource,
  type HttpMethod,
  type HttpResponse,
  type PathResolver,
  ProxyProtocol,
} from '@wvb/deno';
import { toResponse } from './http.ts';

/** The response for a request the protocol failed to serve (default: `500` with the message). */
export type ErrorResponse = (e: Error) => Response;

/** Serves a bundle from the source. */
export interface BundleRoute {
  /** Name of the bundle in the source. */
  bundle: string;
  /**
   * How a request path maps to a file inside the bundle (default: `'directoryIndex'`, i.e.
   * `/about` → `/about/index.html`).
   */
  pathResolver?: PathResolver;
  /**
   * Check each served entry against its xxHash-32 checksum (default: `true`). A mismatch fails the
   * request, which the route answers with its {@link errorResponse}.
   */
  verifyDataChecksum?: boolean;
  /** Seed the bundle was packed with (default: `0`). Must match, or every read mismatches. */
  dataChecksumSeed?: number;
  errorResponse?: ErrorResponse;
}

/** Proxies to another HTTP server — typically a dev server with hot reload. */
export interface ProxyRoute {
  /** Base url of the target server, e.g. `'http://localhost:5173'`. */
  proxy: string;
  errorResponse?: ErrorResponse;
}

export type Route = BundleRoute | ProxyRoute;

/**
 * Mount path → what is served there. `'/'` is the catch-all; the longest matching prefix wins.
 *
 * ```ts
 * routes: {
 *   '/': { bundle: 'app' },
 *   '/docs': { bundle: 'docs', pathResolver: 'htmlExtension' },
 *   '/api': { proxy: 'http://localhost:8080' },
 * }
 * ```
 *
 * A route mounted below the root only works if that app was **built** for the same base path (Vite
 * `base`, Next `basePath`): the mount prefix is stripped before the bundle sees the request, so the
 * app's own absolute urls (`/assets/…`) have to point back at the mount.
 */
export type Routes = Record<string, Route>;

const HTTP_METHODS: ReadonlySet<string> = new Set([
  'get',
  'head',
  'options',
  'post',
  'put',
  'patch',
  'delete',
]);

// The bundle name travels to the core as the uri host (`bundle://<name>/<path>`), and hosts are
// case-folded — a name that is not already url-safe would silently resolve to a different bundle.
const BUNDLE_NAME = /^[a-z0-9][a-z0-9._-]*$/;

/** Host stand-in for proxy routes. Never leaves this module: it only keys the target mapping. */
const PROXY_HOST = 'proxy.wvb';

/** Lower-cased {@link HttpMethod}, or `null` for an unsupported method (callers return 405). */
function toMethod(req: Request): HttpMethod | null {
  const method = req.method.toLowerCase();
  return HTTP_METHODS.has(method) ? (method as HttpMethod) : null;
}

function normalizeHeaders(headers: Headers): Record<string, string> {
  const map: Record<string, string> = {};
  for (const [key, value] of headers.entries()) {
    map[key] = value;
  }
  return map;
}

/** A route with its mount path normalized (`'/'`, or `'/docs'` without a trailing slash). */
export interface Mount {
  readonly mountPath: string;
  readonly route: Route;
}

/** The request path below `mountPath`, or `null` when the request is not under it. */
function relativePath(mountPath: string, pathname: string): string | null {
  if (mountPath === '/') {
    return pathname === '' ? '/' : pathname;
  }
  if (pathname === mountPath) {
    return '/';
  }
  return pathname.startsWith(`${mountPath}/`) ? pathname.slice(mountPath.length) : null;
}

/**
 * Validates {@link Routes} and orders them by match precedence (longest mount path first). Pure —
 * call it before any side effect so a bad config fails fast.
 */
export function normalizeRoutes(routes: Routes): Mount[] {
  const entries = Object.entries(routes);
  if (entries.length === 0) {
    throw new Error('wvb: at least one route is required');
  }
  const mounts = new Map<string, Mount>();
  for (const [path, route] of entries) {
    if (!path.startsWith('/')) {
      throw new Error(`wvb: route path must start with "/" (got ${JSON.stringify(path)})`);
    }
    const trimmed = path.replace(/\/+$/, '');
    const mountPath = trimmed === '' ? '/' : trimmed;
    if (mounts.has(mountPath)) {
      throw new Error(`wvb: duplicate route for ${JSON.stringify(mountPath)}`);
    }
    if ('bundle' in route && !BUNDLE_NAME.test(route.bundle)) {
      throw new Error(
        `wvb: bundle name must be lowercase and url-safe (got ${JSON.stringify(route.bundle)})`
      );
    }
    mounts.set(mountPath, { mountPath, route });
  }
  // `/docs` has to win over `/` for `/docs/guide`.
  return [...mounts.values()].sort((a, b) => b.mountPath.length - a.mountPath.length);
}

interface Handler {
  readonly mountPath: string;
  readonly errorResponse?: ErrorResponse;
  /** `path` is below the mount and carries the query string. */
  handle(req: Request, method: HttpMethod, path: string): Promise<HttpResponse>;
}

/** The request body, or `undefined` when the request has none (GET/HEAD never do). */
async function readBody(req: Request): Promise<Uint8Array<ArrayBuffer> | undefined> {
  if (req.body == null) {
    return undefined;
  }
  const body = new Uint8Array(await req.arrayBuffer());
  return body.byteLength > 0 ? body : undefined;
}

function toHandler({ mountPath, route }: Mount, source: BundleSource): Handler {
  const { errorResponse } = route;
  if ('proxy' in route) {
    const protocol = new ProxyProtocol({ [PROXY_HOST]: route.proxy });
    return {
      mountPath,
      errorResponse,
      handle: async (req, method, path) =>
        protocol.handle(
          method,
          `proxy://${PROXY_HOST}${path}`,
          normalizeHeaders(req.headers),
          await readBody(req)
        ),
    };
  }
  const protocol = new BundleProtocol(source, {
    pathResolver: route.pathResolver,
    verifyDataChecksum: route.verifyDataChecksum,
    dataChecksumSeed: route.dataChecksumSeed,
  });
  return {
    mountPath,
    errorResponse,
    handle: (req, method, path) =>
      protocol.handle(method, `bundle://${route.bundle}${path}`, normalizeHeaders(req.headers)),
  };
}

const defaultErrorResponse: ErrorResponse = e => new Response(e.message, { status: 500 });

/** Builds the `Deno.serve` handler that serves each request from the route mounted at its path. */
export function createHandler(
  mounts: Mount[],
  source: BundleSource
): (req: Request) => Promise<Response> {
  const handlers = mounts.map(mount => toHandler(mount, source));
  return async (req: Request): Promise<Response> => {
    const { pathname, search } = new URL(req.url);
    for (const handler of handlers) {
      const path = relativePath(handler.mountPath, pathname);
      if (path == null) {
        continue;
      }
      const method = toMethod(req);
      if (method == null) {
        return new Response('Method Not Allowed', { status: 405 });
      }
      try {
        return toResponse(await handler.handle(req, method, `${path}${search}`));
      } catch (e) {
        const error = e instanceof Error ? e : new Error(String(e));
        return (handler.errorResponse ?? defaultErrorResponse)(error);
      }
    }
    return new Response('Not Found', { status: 404 });
  };
}
