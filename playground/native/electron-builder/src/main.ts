import path from 'node:path';
import { bundleProtocol, webviewBundle } from '@wvb/electron';
import { app, BrowserWindow } from 'electron';

const wvb = webviewBundle({
  source: app.isPackaged
    ? undefined
    : { builtinDir: path.join(import.meta.dirname, '..', '.wvb', 'builtin', 'bundles') },
  protocols: [bundleProtocol('app')],
  updater: {
    remote: {
      endpoint: __WVB_PLAYGROUND_REMOTE_ENDPOINT__,
    },
  },
});

async function bootstrap() {
  await wvb.whenProtocolRegistered();

  const win = new BrowserWindow({
    width: 800,
    height: 600,
    webPreferences: {
      contextIsolation: true,
      preload: path.join(import.meta.dirname, 'preload.cjs'),
    },
  });
  await win.loadURL('app://testbed.wvb');
}

void bootstrap();
