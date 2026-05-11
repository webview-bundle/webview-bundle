import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { bundleProtocol, wvb } from '@wvb/electron';
import { app, BrowserWindow } from 'electron';

const dirname = path.dirname(fileURLToPath(import.meta.url));

wvb({
  source: {
    builtinDir: path.join(dirname, '..', '..', '..', 'fixtures', 'bundles'),
  },
  protocols: [bundleProtocol('app')],
});

let mainWindow: BrowserWindow;

(async () => {
  await app.whenReady();
  mainWindow = new BrowserWindow();
  await mainWindow.loadURL('app://next.wvb');
})();
