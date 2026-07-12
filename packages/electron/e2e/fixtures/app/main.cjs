const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { bundleProtocol, wvb } = require('@wvb/electron');

const instance = wvb({
  source: {
    builtinDir: path.join(__dirname, 'bundles'),
    remoteDir: path.join(os.tmpdir(), 'wvb-electron-e2e-remote'),
  },
  protocols: [bundleProtocol('app')],
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
  await window.loadURL('app://hacker-news.wvb');
});

app.on('window-all-closed', () => app.quit());
