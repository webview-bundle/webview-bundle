import '@fontsource-variable/inter';
import '@fontsource-variable/jetbrains-mono';
import './styles.css';

import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { createRouter } from './router';

const router = createRouter();
const rootEl = document.getElementById('root');

if (rootEl) {
  // Each route is prerendered to static HTML (see scripts/prerender.mjs), which
  // gives an instant, crawlable first paint. We then resolve the initial matches
  // and mount React over it. We render (not hydrate) on purpose: standalone
  // TanStack Router — without TanStack Start — doesn't emit the dehydration
  // payload that clean hydration needs, and these pages are tiny, so a fresh
  // client render is effectively free and avoids any hydration mismatch. React
  // swaps in identical markup in a single commit, so there is no visible flash.
  router.load().then(() => {
    createRoot(rootEl).render(
      <StrictMode>
        <RouterProvider router={router} />
      </StrictMode>
    );
  });
}
