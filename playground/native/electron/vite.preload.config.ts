import path from 'node:path';
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    emptyOutDir: false,
    lib: {
      entry: path.resolve(import.meta.dirname, 'src/preload.ts'),
      formats: ['cjs'],
      fileName: () => 'preload.cjs',
    },
    rolldownOptions: {
      platform: 'node',
      external: ['electron', '@wvb/node', /^node:/],
    },
  },
});
