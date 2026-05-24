import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT_DIR = path.join(path.dirname(fileURLToPath(import.meta.url)), '..');

export const GITHUB_REPO = {
  owner: 'webview-bundle',
  name: 'webview-bundle',
};

export const GIT_SIGNATURE = {
  name: 'Seokju Na',
  email: 'seokju.me@gmail.com',
};
