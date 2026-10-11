import os from 'node:os';
import path from 'node:path';

export function getDefaultBaseDir(): string {
  return path.join(os.homedir(), '.wvb', 'local');
}
