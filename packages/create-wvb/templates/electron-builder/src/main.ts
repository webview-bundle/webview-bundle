import path from 'node:path';
import { app, BrowserWindow } from 'electron';
import { bundleProtocol, proxyProtocol, wvb } from '@wvb/electron';

const BUNDLE_NAME = '{{bundleName}}';
const APP_URL = `app://${BUNDLE_NAME}.wvb/`;

/**
 * `npm run dev` passes `--dev` to serve the Vite dev server instead of the packed bundle.
 * `app.isPackaged` gates it so a shipped build can never be pointed at a dev server.
 */
const devServerUrl =
  !app.isPackaged && process.argv.includes('--dev')
    ? (process.env.WVB_DEV_SERVER_URL ?? 'http://localhost:5173')
    : null;

/**
 * Must run at module top level: `wvb()` calls `protocol.registerSchemesAsPrivileged()`,
 * which Electron only honors before the `ready` event fires.
 */
const instance = wvb({
  protocols: [
    devServerUrl != null
      ? proxyProtocol('app', { hosts: { [`${BUNDLE_NAME}.wvb`]: devServerUrl } })
      : bundleProtocol('app'),
  ],
});

async function createWindow(): Promise<void> {
  await instance.whenProtocolRegistered();

  const win = new BrowserWindow({
    width: 1024,
    height: 768,
    webPreferences: {
      preload: path.join(app.getAppPath(), 'dist', 'preload.cjs'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  await win.loadURL(APP_URL);
}

app.whenReady().then(createWindow).catch(handleFatal);

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow().catch(handleFatal);
  }
});

function handleFatal(error: unknown): void {
  console.error(error);
  app.quit();
}

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
