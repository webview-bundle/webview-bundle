import { withWebviewBundle } from '@wvb/electron-builder';

export default withWebviewBundle({
  appId: 'dev.wvb.playground.electron-builder',
  productName: 'WebviewBundlePlaygroundElectronBuilder',
  asar: true,
  asarUnpack: ['**/node_modules/@wvb/node/**'],
  directories: {
    output: 'out',
  },
  mac: { target: 'dir' },
  linux: { target: 'dir' },
  win: { target: 'dir' },
});
