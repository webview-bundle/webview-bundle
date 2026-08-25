import { withWebviewBundle } from '@wvb/electron-builder';
import type { Configuration } from 'electron-builder';

// This file must keep the name "electron-builder.ts": electron-builder only auto-discovers
// electron-builder.{yml,yaml,json,json5,toml,js,cjs,ts}. A name like "electron-builder.config.ts"
// is silently ignored, and the app would then package with no bundles at all. A "build" field in
// package.json also takes precedence over this file — don't add one.
const config: Configuration = {
  appId: 'com.example.{{bundleName}}',
  files: ['**/*', '!web{,/**/*}', '!bundles{,/**/*}', '!.wvb{,/**/*}', '!wvb.config.ts', '!README.md'],
  asar: true,
  // The native @wvb/node addon is published as per-platform packages (@wvb/node-darwin-arm64,
  // @wvb/node-win32-x64-msvc, …). The ".node" binary lives in those, not in @wvb/node itself,
  // and a native binary cannot be loaded from inside the asar — so unpack the whole family.
  asarUnpack: ['**/node_modules/@wvb/node*/**'],
  mac: { target: 'dmg' },
  win: { target: 'nsis' },
  linux: { target: 'AppImage' },
};

// Composes an `afterPack` hook that installs the builtin .wvb bundles into Resources/bundles.
export default withWebviewBundle(config);
