import { createRouter as createTanStackRouter } from '@tanstack/react-router';
import { routeTree } from './routeTree.gen';

type RouterOptions = Parameters<typeof createTanStackRouter>[0];

/**
 * Single router factory used by both entries:
 *  - the client (browser history, the default) hydrates the prerendered HTML
 *  - the prerender (memory history, passed in `opts`) renders each route to a
 *    string at build time.
 */
export function createRouter(opts?: Pick<RouterOptions, 'history'>) {
  return createTanStackRouter({
    routeTree,
    defaultPreload: 'intent',
    // NOTE: scrollRestoration is intentionally left off. It injects an inline
    // <script> into the SSR output that self-removes on the client, which would
    // break hydration of our prerendered HTML. This app scrolls inside panels
    // (overflow-y-auto), so document-level restoration adds little anyway.
    ...opts,
  });
}

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createRouter>;
  }
}
