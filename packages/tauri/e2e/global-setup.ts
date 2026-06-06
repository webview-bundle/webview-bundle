import { spawn } from 'node:child_process';
import path from 'node:path';

const srcTauriDir = path.join(import.meta.dirname, 'fixtures', 'app', 'src-tauri');

export async function setup(): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const child = spawn('cargo', ['build', '--release'], {
      cwd: srcTauriDir,
      stdio: 'inherit',
    });

    child.on('error', reject);
    child.on('exit', code => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(new Error(`cargo build --release failed with exit code ${code ?? 'unknown'}`));
    });
  });
}
