import { defineConfig } from '@wvb/cli';
import { loadEnv } from '@wvb-playground/env';

const env = loadEnv();

export default defineConfig({
  remote: {
    endpoint: env.remote.endpoint,
  },
});
