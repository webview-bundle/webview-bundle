import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { app, BrowserWindow } from 'electron';
import serve from 'electron-serve';

const dirname = path.dirname(fileURLToPath(import.meta.url));

serve({
  scheme: 'app',
  directory: path.resolve(dirname, '..', '..', '..', 'fixtures', 'next', 'out'),
});

let mainWindow: BrowserWindow;

(async () => {
  await app.whenReady();
  mainWindow = new BrowserWindow();
  await mainWindow.loadURL('app://-');
})();
