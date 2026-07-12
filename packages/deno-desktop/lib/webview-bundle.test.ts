import { assert, assertEquals, assertThrows } from '@std/assert';
import { fromFileUrl } from '@std/path';
import { loadLib } from '@wvb/deno';
import { type Routes, webviewBundle } from './mod.ts';

// Use the locally-built cdylib + the committed builtin fixture (bundle "app" v1.0.0).
const ext = Deno.build.os === 'windows' ? 'dll' : Deno.build.os === 'darwin' ? 'dylib' : 'so';
const prefix = Deno.build.os === 'windows' ? '' : 'lib';
const DYLIB = fromFileUrl(
  new URL(`../../../target/release/${prefix}wvb_deno.${ext}`, import.meta.url)
);
const BUILTIN_DIR = fromFileUrl(new URL('../fixtures/builtin', import.meta.url));

// Load the native library once at module top level — not inside a test — so Deno's per-test
// resource-leak sanitizer doesn't flag the intentionally process-lifetime FFI handle.
loadLib(DYLIB);

function makeApp(routes: Routes = { '/': { bundle: 'app' } }) {
  return webviewBundle({
    source: { builtinDir: BUILTIN_DIR, remoteDir: Deno.makeTempDirSync({ prefix: 'wvb-test-' }) },
    routes,
  });
}

Deno.test('webviewBundle.fetch serves the builtin bundle index at the root', async () => {
  const app = makeApp();
  const res = await app.fetch(new Request('http://127.0.0.1/'));
  assertEquals(res.status, 200);
  assertEquals(res.headers.get('content-type'), 'text/html');
  assert((await res.text()).includes('Pagination with SSG'));
});

Deno.test('webviewBundle.fetch forwards Range (206) and returns 404', async () => {
  const app = makeApp();

  const ranged = await app.fetch(
    new Request('http://127.0.0.1/build.png', { headers: { Range: 'bytes=0-99' } })
  );
  assertEquals(ranged.status, 206);
  assertEquals((await ranged.arrayBuffer()).byteLength, 100);

  const missing = await app.fetch(new Request('http://127.0.0.1/nope'));
  assertEquals(missing.status, 404);
});

Deno.test('a route forwards its path resolver to the bundle', async () => {
  // The default (directoryIndex) looks for `/index/index.html`, which the bundle does not have.
  const byDirectory = makeApp();
  assertEquals((await byDirectory.fetch(new Request('http://127.0.0.1/index'))).status, 404);

  // `htmlExtension` resolves `/index` to `/index.html`.
  const byExtension = makeApp({ '/': { bundle: 'app', pathResolver: 'htmlExtension' } });
  const res = await byExtension.fetch(new Request('http://127.0.0.1/index'));
  assertEquals(res.status, 200);
  assertEquals(res.headers.get('content-type'), 'text/html');
});

Deno.test('a bundle mounted below the root has its mount path stripped', async () => {
  const app = makeApp({ '/docs': { bundle: 'app' } });

  // `/docs` and `/docs/` both reach the bundle's `/index.html`.
  assertEquals((await app.fetch(new Request('http://127.0.0.1/docs'))).status, 200);
  const index = await app.fetch(new Request('http://127.0.0.1/docs/'));
  assertEquals(index.status, 200);
  assert((await index.text()).includes('Pagination with SSG'));

  // Nested paths keep working below the mount.
  const nested = await app.fetch(new Request('http://127.0.0.1/docs/category/2/index.html'));
  assertEquals(nested.status, 200);

  // Nothing is mounted at the root.
  assertEquals((await app.fetch(new Request('http://127.0.0.1/'))).status, 404);
  // `/docsxyz` is not below the `/docs` mount.
  assertEquals((await app.fetch(new Request('http://127.0.0.1/docsxyz'))).status, 404);
});

Deno.test('the longest matching mount path wins', async () => {
  const app = makeApp({
    '/': { bundle: 'app' },
    // Same bundle, but `exact` does not rewrite `/` to `/index.html`.
    '/docs': { bundle: 'app', pathResolver: 'exact' },
  });
  assertEquals(app.routePaths, ['/docs', '/']);

  // Served by the `/docs` mount → `/` → no index rewrite → 404.
  assertEquals((await app.fetch(new Request('http://127.0.0.1/docs/'))).status, 404);
  // Served by the `/docs` mount → `/index.html` → 200.
  assertEquals((await app.fetch(new Request('http://127.0.0.1/docs/index.html'))).status, 200);
  // Served by the root mount (directoryIndex) → 200.
  assertEquals((await app.fetch(new Request('http://127.0.0.1/'))).status, 200);
});

Deno.test('a proxy route forwards the path and query to the target', async () => {
  const seen: string[] = [];
  const server = Deno.serve({ hostname: '127.0.0.1', port: 0, onListen: () => {} }, req => {
    const { pathname, search } = new URL(req.url);
    seen.push(`${pathname}${search}`);
    return new Response('from the dev server', { headers: { 'content-type': 'text/plain' } });
  });
  try {
    const app = makeApp({ '/dev': { proxy: `http://127.0.0.1:${server.addr.port}` } });
    const res = await app.fetch(new Request('http://127.0.0.1/dev/index.html?foo=bar'));
    assertEquals(res.status, 200);
    assertEquals(await res.text(), 'from the dev server');
    // The mount path is stripped, the rest (with the query) is passed on.
    assertEquals(seen, ['/index.html?foo=bar']);
  } finally {
    await server.shutdown();
  }
});

Deno.test('a proxy route forwards the request body to the target', async () => {
  let received: { method: string; body: string } | null = null;
  const server = Deno.serve({ hostname: '127.0.0.1', port: 0, onListen: () => {} }, async req => {
    received = { method: req.method, body: await req.text() };
    return new Response('ok');
  });
  try {
    const app = makeApp({ '/api': { proxy: `http://127.0.0.1:${server.addr.port}` } });
    const res = await app.fetch(
      new Request('http://127.0.0.1/api/submit', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ hello: 'world' }),
      })
    );
    assertEquals(res.status, 200);
    assertEquals(received, { method: 'POST', body: '{"hello":"world"}' });
  } finally {
    await server.shutdown();
  }
});

Deno.test('a proxy route forwards maxCacheBytes to the protocol', async () => {
  let served = 0;
  const server = Deno.serve({ hostname: '127.0.0.1', port: 0, onListen: () => {} }, () => {
    served += 1;
    // Answer 304 from the second request on, as a dev server would for an unchanged asset.
    return served === 1
      ? new Response('asset', { headers: { etag: '"v1"' } })
      : new Response(null, { status: 304, headers: { etag: '"v1"' } });
  });
  try {
    // With the cache off, the 304 reaches the client instead of the body the proxy last saw.
    const app = makeApp({
      '/': { proxy: `http://127.0.0.1:${server.addr.port}`, maxCacheBytes: 0 },
    });
    assertEquals((await app.fetch(new Request('http://127.0.0.1/app.js'))).status, 200);
    assertEquals((await app.fetch(new Request('http://127.0.0.1/app.js'))).status, 304);
  } finally {
    await server.shutdown();
  }
});

Deno.test('an unreachable proxy target surfaces as a 500', async () => {
  const app = makeApp({ '/': { proxy: 'http://127.0.0.1:59999' } });
  const res = await app.fetch(new Request('http://127.0.0.1/index.html'));
  assertEquals(res.status, 500);
});

Deno.test('an invalid routes config fails fast', () => {
  assertThrows(() => makeApp({}), Error, 'at least one route');
  assertThrows(() => makeApp({ docs: { bundle: 'app' } }), Error, 'must start with "/"');
  assertThrows(
    () => makeApp({ '/docs': { bundle: 'app' }, '/docs/': { bundle: 'app' } }),
    Error,
    'duplicate route'
  );
  assertThrows(() => makeApp({ '/': { bundle: 'App' } }), Error, 'lowercase and url-safe');
});

Deno.test('webviewBundle exposes source + routePaths', () => {
  const app = makeApp();
  assert(app.source != null);
  assertEquals(app.routePaths, ['/']);
  assertEquals(app.remote, null);
  assertEquals(app.updater, null);
});
