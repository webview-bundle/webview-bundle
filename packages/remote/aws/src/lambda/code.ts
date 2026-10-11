import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import type { WebviewBundleRemoteConfig } from './remote.js';

export function getOriginRequestHandleCodeFilePath(): string {
  const require = createRequire(import.meta.url);
  const packagePath = path.dirname(require.resolve('@wvb/remote-aws/package.json'));
  const codeFilePath = path.join(packagePath, 'dist', 'assets', 'origin-request.mjs');

  return codeFilePath;
}

export function generateOriginRequestHandlerCode(config: WebviewBundleRemoteConfig): string {
  const code = fs.readFileSync(getOriginRequestHandleCodeFilePath(), 'utf8');

  return code.replaceAll('__WVB_REMOTE_CONFIG__', () => JSON.stringify(config));
}
