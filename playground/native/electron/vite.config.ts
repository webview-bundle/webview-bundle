import path from 'node:path';
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    lib: {
      entry: path.resolve(import.meta.dirname, 'src/main.ts'),
      formats: ['es'],
      fileName: () => 'main.mjs',
    },
    rolldownOptions: {
      platform: 'node',
      external: ['electron', '@wvb/node', /^node:/],
    },
  },
});
