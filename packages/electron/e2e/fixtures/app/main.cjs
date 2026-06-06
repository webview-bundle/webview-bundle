// Minimal Electron main process for the e2e fixture app.
//
// It mirrors examples/electron-forge-vite: register the `app://` bundle protocol against a
// builtin bundle directory, then open a window pointed at the bundle. The bundle served here is
// the packed Next.js SSG output (see e2e/prepare-bundle.ts), so the e2e can assert that real
// web content is delivered through the bundle protocol into a live BrowserWindow.
const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { bundleProtocol, wvb } = require('@wvb/electron');

const instance = wvb({
  source: {
    builtinDir: path.join(__dirname, 'bundles'),
    remoteDir: path.join(os.tmpdir(), 'wvb-electron-e2e-remote'),
  },
  protocols: [bundleProtocol('app', { onError: e => console.error('[wvb]', e) })],
});

app.whenReady().then(async () => {
  // Wait until the custom protocol is actually registered before navigating to it.
  await instance.whenProtocolRegistered();
  const window = new BrowserWindow({
    width: 1024,
    height: 768,
    show: false,
    webPreferences: { contextIsolation: true, nodeIntegration: false },
  });
  await window.loadURL('app://next.wvb');
});

app.on('window-all-closed', () => app.quit());
