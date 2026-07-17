// Static-site generation step. Runs after the client + SSR builds:
//   1. client build  -> dist/  (hashed assets + index.html template)
//   2. ssr build      -> dist-ssr/entry-server.js
//   3. this script    -> writes dist/<route>/index.html for every known route
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { getStaticPaths, render } from '../dist-ssr/entry-server.js';

const distDir = resolve(process.cwd(), 'dist');
const template = await readFile(join(distDir, 'index.html'), 'utf8');

function escapeHtml(value) {
  return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function buildPage(appHtml, title) {
  let html = template;
  html = html.includes('<!--app-html-->')
    ? html.replace('<!--app-html-->', appHtml)
    : html.replace(/(<div id="root">)(<\/div>)/, `$1${appHtml}$2`);
  html = html.replace(/<title>[\s\S]*?<\/title>/, `<title>${escapeHtml(title)}</title>`);
  return html;
}

const paths = getStaticPaths();
for (const { url, title } of paths) {
  const appHtml = await render(url);
  const outPath = url === '/' ? join(distDir, 'index.html') : join(distDir, url, 'index.html');
  await mkdir(dirname(outPath), { recursive: true });
  await writeFile(outPath, buildPage(appHtml, title), 'utf8');
}

// SPA fallback for any deep link that wasn't prerendered: ship an empty shell
// that the client boots and routes on its own.
await writeFile(join(distDir, '404.html'), buildPage('', 'BUNDLE // news'), 'utf8');

console.log(`✓ prerendered ${paths.length} routes + 404.html → dist/`);
