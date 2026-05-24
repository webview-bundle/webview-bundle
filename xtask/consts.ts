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

/**
 * Prefix of the commit message `prepare-release` writes for the version-bump commit. Used both to
 * build that message and to recognize (and skip) release commits during change detection.
 */
export const RELEASE_COMMIT_PREFIX = 'release:';
