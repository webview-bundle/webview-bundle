import fs from 'node:fs/promises';

export async function pathExists(path: string): Promise<boolean> {
  try {
    await fs.access(path);
    return true;
  } catch (e) {
    if ((e as any)?.code === 'ENOENT') {
      return false;
    }
    throw e;
  }
}
