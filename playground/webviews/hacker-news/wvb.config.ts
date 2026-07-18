import { defineConfig } from '@wvb/cli';
import { cloudflareRemote } from '@wvb/remote-cloudflare';
import { loadEnv } from '@wvb-playground/env';

const env = loadEnv();

export default defineConfig({
  remote: {
    ...cloudflareRemote({
      bucket: env.cloudflare.bucket,
      accountId: env.cloudflare.accountId,
      kvNamespaceId: env.cloudflare.kvNamespaceId,
      cloudflareConfig: {
        apiToken: env.cloudflare.apiToken,
      },
      aws: {
        region: 'auto',
        credentials: {
          accessKeyId: env.cloudflare.accessKeyId,
          secretAccessKey: env.cloudflare.secretAccessKey,
        },
      },
    }),
    packBeforeUpload: true,
    bundleName: 'hacker-news',
  },
});
