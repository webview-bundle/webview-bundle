const os = require('node:os');
const path = require('node:path');
const { app, BrowserWindow } = require('electron');
const { bundleProtocol, webviewBundle } = require('@wvb/electron');

const wvb = webviewBundle({
  source: {
    builtinDir: path.join(__dirname, 'bundles'),
    remoteDir: path.join(os.tmpdir(), 'wvb-electron-e2e-remote'),
  },
  protocols: [bundleProtocol('app')],
});

wvb.ready().then(async () => {
  const window = new BrowserWindow({
    width: 1024,
    height: 768,
    show: false,
    webPreferences: { contextIsolation: true, nodeIntegration: false },
  });
  await window.loadURL('app://hacker-news.wvb');
});

app.on('window-all-closed', () => app.quit());
