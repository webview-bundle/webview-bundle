import { createMemoryHistory, RouterProvider } from '@tanstack/react-router';
import { renderToString } from 'react-dom/server';
import { allAuthors, posts } from './data';
import { createRouter } from './router';

/** Render a single route URL to an HTML string (used by scripts/prerender.mjs). */
export async function render(url: string): Promise<string> {
  const router = createRouter({ history: createMemoryHistory({ initialEntries: [url] }) });
  await router.load();
  return renderToString(<RouterProvider router={router} />);
}

export interface StaticPath {
  url: string;
  title: string;
}

const SITE = 'WEBVIEW BUNDLE // news';

/** Every route to prerender: the feed, each post, and each author profile. */
export function getStaticPaths(): StaticPath[] {
  const paths: StaticPath[] = [{ url: '/', title: SITE }];
  for (const p of posts) {
    paths.push({ url: `/post/${p.id}`, title: `${p.title} · ${SITE}` });
  }
  for (const author of allAuthors()) {
    paths.push({ url: `/u/${author}`, title: `${author} · ${SITE}` });
  }
  return paths;
}
