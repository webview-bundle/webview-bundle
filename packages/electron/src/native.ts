import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { loadBinding, type WvbNodeBinding } from '@wvb/node/binding';

function nodeBindingsDir(): string {
  return path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'node-bindings');
}

export const wvbNode: WvbNodeBinding = loadBinding(nodeBindingsDir());
