// Protocol handlers (mirrors @wvb/electron's protocol.ts). Deno desktop serves over local HTTP
// (single origin), so a protocol's `scheme` is used as the bundle name / local host, and each
// handler maps the incoming `http://127.0.0.1/<path>` request to the binding's URI form.
import {
  BundleProtocol,
  type BundleSource,
  type HttpMethod,
  LocalProtocol,
  toResponse,
} from '@wvb/deno';

export interface ProtocolHandler {
  handle(req: Request): Promise<Response>;
}

export interface ProtocolHandlerBuildContext {
  source: BundleSource;
}
export type ProtocolHandlerBuild = (
  ctx: ProtocolHandlerBuildContext
) => ProtocolHandler | Promise<ProtocolHandler>;

export interface ProtocolOptions {
  onError?: (e: Error) => void;
}

export interface Protocol {
  /** In Deno desktop (single-origin HTTP) this is the bundle name / local host served at the root. */
  scheme: string;
  handler: ProtocolHandler | ProtocolHandlerBuild;
  options?: ProtocolOptions;
}

function normalizeHeaders(headers: Headers): Record<string, string> {
  const map: Record<string, string> = {};
  for (const [key, value] of headers.entries()) {
    map[key] = value;
  }
  return map;
}

const HTTP_METHODS: ReadonlySet<string> = new Set([
  'get',
  'head',
  'options',
  'post',
  'put',
  'patch',
  'delete',
]);

/** Lower-cased {@link HttpMethod}, or `null` for an unsupported method (callers return 405). */
function toMethod(req: Request): HttpMethod | null {
  const method = req.method.toLowerCase();
  return HTTP_METHODS.has(method) ? (method as HttpMethod) : null;
}

const METHOD_NOT_ALLOWED = (): Response => new Response('Method Not Allowed', { status: 405 });

export interface BundleProtocolConfig extends ProtocolOptions {}

/** Serve a builtin bundle (named `scheme`) at the HTTP root. */
export function bundleProtocol(scheme: string, config: BundleProtocolConfig = {}): Protocol {
  return {
    scheme,
    handler: ({ source }) => {
      const bundle = new BundleProtocol(source);
      return {
        handle: async (req: Request): Promise<Response> => {
          const method = toMethod(req);
          if (method == null) {
            return METHOD_NOT_ALLOWED();
          }
          const { pathname, search } = new URL(req.url);
          const resp = await bundle.handle(
            method,
            `bundle://${scheme}${pathname}${search}`,
            normalizeHeaders(req.headers)
          );
          return toResponse(resp);
        },
      };
    },
    options: config,
  };
}

type Hosts = Record<string, string>;

export interface LocalProtocolConfig extends ProtocolOptions {
  hosts: Hosts | (() => Hosts | Promise<Hosts>);
}

/** Proxy to a local dev server (hot reload), mapping `scheme` host → URL. */
export function localProtocol(scheme: string, config: LocalProtocolConfig): Protocol {
  const { hosts, ...options } = config;
  return {
    scheme,
    handler: async () => {
      const resolved = typeof hosts === 'function' ? await hosts() : hosts;
      const local = new LocalProtocol(resolved);
      return {
        handle: async (req: Request): Promise<Response> => {
          const method = toMethod(req);
          if (method == null) {
            return METHOD_NOT_ALLOWED();
          }
          const { pathname, search } = new URL(req.url);
          const resp = await local.handle(
            method,
            `app://${scheme}${pathname}${search}`,
            normalizeHeaders(req.headers)
          );
          return toResponse(resp);
        },
      };
    },
    options,
  };
}
