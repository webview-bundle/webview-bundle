import { Buffer } from 'node:buffer';
import {
  BundleProtocol,
  type BundleResolverOptions,
  type BundleSource,
  type HttpMethod,
  type HttpResponse,
  type PathResolver,
  ProxyProtocol,
} from '@wvb/node';
import type { Protocol as ElectronProtocol, Privileges } from 'electron';
import { app, protocol as electronProtocol } from 'electron';
import { makeError } from './utils.js';

export interface ProtocolHandler {
  handle(req: Request): Promise<Response>;
}

export interface ProtocolOptions {
  protocol?: () => ElectronProtocol;
  privileges?: Privileges;
  onError?: (e: Error) => void;
}

export interface ProtocolHandlerBuildContext {
  source: BundleSource;
}
export type ProtocolHandlerBuild = (
  ctx: ProtocolHandlerBuildContext
) => ProtocolHandler | Promise<ProtocolHandler>;

export interface Protocol {
  scheme: string;
  handler: ProtocolHandler | ProtocolHandlerBuild;
  options?: ProtocolOptions;
}

const DEFAULT_PRIVILEGES: Privileges = {
  standard: true,
  secure: true,
  bypassCSP: true,
  allowServiceWorkers: true,
  supportFetchAPI: true,
  corsEnabled: true,
  stream: false,
  codeCache: true,
};

export async function registerProtocol(protocol: Protocol, source: BundleSource): Promise<void> {
  const { scheme, handler, options = {} } = protocol;
  const { protocol: getProtocol, privileges, onError } = options;

  electronProtocol.registerSchemesAsPrivileged([
    {
      scheme,
      privileges: { ...DEFAULT_PRIVILEGES, ...privileges },
    },
  ]);

  await app.whenReady();
  const h = typeof handler === 'function' ? await handler({ source }) : handler;
  const p = getProtocol?.() ?? electronProtocol;
  if (typeof p.handle === 'function') {
    p.handle(scheme, async req => {
      try {
        const resp = await h.handle(req);
        return resp;
      } catch (e) {
        const error = makeError(e);
        onError?.(error);
        return new Response(error.message, { status: 500 });
      }
    });
  } else {
    // support for electron < 25
    p.registerBufferProtocol(scheme, async (req, callback) => {
      const request = new Request(req.url, {
        method: req.method,
        headers: req.headers,
      });
      try {
        const response = await h.handle(request);
        callback({
          statusCode: response.status,
          headers: normalizeHeaders(response.headers),
          data: Buffer.from(await response.arrayBuffer()),
        });
      } catch (e) {
        onError?.(makeError(e));
        callback({ error: -2 });
      }
    });
  }
}

type Hosts = Record<string, string>;

/**
 * Resolves the proxy target for a request uri (`null` to not proxy). The path and query of the
 * request are appended to whatever it returns.
 */
export type ProxyResolver = (uri: string) => Promise<string | null>;

export type ProxyProtocolConfig = ProtocolOptions &
  (
    | {
        /**
         * Host → target url mapping, or a function returning one — evaluated once, when the handler
         * is built (for a mapping that is only known by then, e.g. a dev server port).
         */
        hosts: Hosts | (() => Hosts | Promise<Hosts>);
        resolver?: never;
      }
    | {
        /** Called with each request uri, for routing that depends on the request. */
        resolver: ProxyResolver;
        hosts?: never;
      }
  );

/** Proxy the scheme to another server (a dev server with hot reload). */
export function proxyProtocol(scheme: string, config: ProxyProtocolConfig): Protocol {
  const { hosts, resolver, ...options } = config;
  const protocol: Protocol = {
    scheme,
    handler: async () => {
      const proxy = new ProxyProtocol(
        resolver ?? (typeof hosts === 'function' ? await hosts() : (hosts as Hosts))
      );
      return {
        handle: async req => {
          const method = req.method.toLowerCase() as HttpMethod;
          const resp = await proxy.handle(
            method,
            req.url,
            normalizeHeaders(req.headers),
            await readBody(req)
          );
          return makeResponse(resp);
        },
      };
    },
    options,
  };
  return protocol;
}

/** The request body, or `undefined` when it has none — a proxied POST/PUT/PATCH carries one. */
async function readBody(req: Request): Promise<Buffer | undefined> {
  if (req.body == null) {
    return undefined;
  }
  const body = Buffer.from(await req.arrayBuffer());
  return body.byteLength > 0 ? body : undefined;
}

export interface BundleProtocolConfig extends ProtocolOptions {
  /**
   * How the bundle name is resolved from the request uri (default: the first hostname segment,
   * e.g. `app://my-app/index.html` -> bundle "my-app").
   */
  bundleResolver?: BundleResolverOptions;
  /**
   * How the file path in the bundle is resolved from the request uri (default: `'directoryIndex'`,
   * i.e. `/about` -> `/about/index.html`).
   */
  pathResolver?: PathResolver;
}

/** Serve bundles from the source over the scheme. */
export function bundleProtocol(scheme: string, config: BundleProtocolConfig = {}): Protocol {
  const { bundleResolver, pathResolver, ...options } = config;
  const protocol: Protocol = {
    scheme,
    handler: ({ source }) => {
      const bundle = new BundleProtocol(source, { bundleResolver, pathResolver });
      return {
        handle: async req => {
          const method = req.method.toLowerCase() as HttpMethod;
          const resp = await bundle.handle(method, req.url, normalizeHeaders(req.headers));
          return makeResponse(resp);
        },
      };
    },
    options,
  };
  return protocol;
}

function normalizeHeaders(headers: Headers): Record<string, string> {
  const map: Record<string, string> = {};
  for (const [key, value] of headers.entries()) {
    map[key] = value;
  }
  return map;
}

function makeResponse(resp: HttpResponse): Response {
  const { status, headers: respHeaders, body } = resp;
  const headers = new Headers();
  for (const [key, value] of Object.entries(respHeaders)) {
    headers.set(key, value);
  }
  return new Response(body as any, { status, headers });
}
