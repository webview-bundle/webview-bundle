import path from 'node:path';
import { defineRuntime, loadEnv } from '@wvb-playground/env';
import { defineConfig } from 'vite';

const env = loadEnv();

export default defineConfig({
  build: {
    lib: {
      entry: path.resolve(import.meta.dirname, 'src/main.ts'),
      formats: ['es'],
      fileName: () => 'main.mjs',
    },
    rolldownOptions: {
      platform: 'node',
      external: ['electron', /^@wvb\/node/, /^node:/],
    },
  },
  define: {
    ...defineRuntime(env),
  },
});
