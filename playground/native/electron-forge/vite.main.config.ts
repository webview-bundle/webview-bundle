import { defineRuntime, loadEnv } from '@wvb-playground/env';
import { defineConfig } from 'vite';

const env = loadEnv();

export default defineConfig({
  build: {
    rollupOptions: {
      external: [/^@wvb\/node/],
    },
  },
  define: {
    ...defineRuntime(env),
  },
});
