import path from 'node:path';

export const PKG_DIR = path.join(import.meta.dirname, '..');
export const ROOT_DIR = path.join(PKG_DIR, '..', '..');
