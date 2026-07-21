import { defineConfig } from 'vite';

export default defineConfig({
  // Must stay root-absolute. The bundle protocol takes the bundle name from the URL *host*
  // (`app://<name>.wvb/assets/x.js` -> bundle `<name>`, file `/assets/x.js`), so root-relative
  // asset URLs always land in the right bundle. A relative base ('./') breaks on nested routes:
  // from a document at `app://<name>.wvb/about/`, './assets/x.js' resolves to '/about/assets/x.js'.
  base: '/',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
});
