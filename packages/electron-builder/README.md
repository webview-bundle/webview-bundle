# @wvb/electron-builder

[electron-builder](https://www.electron.build/) integration for [webview-bundle](https://github.com/webview-bundle/webview-bundle).

It installs your **builtin** Webview Bundles into the packaged app at build time — downloading them
from your remote and/or packing them from local workspaces (as configured in `wvb.config.ts`) — and
places them in the app's `Resources/bundles` directory, exactly where [`@wvb/electron`](../electron)
reads them at runtime.

This is the electron-builder counterpart to [`@wvb/electron-forge`](../electron-forge). Because
electron-builder has no plugin system, the integration is delivered as a Higher-order Config (HoC)
that wraps your electron-builder configuration — the same pattern used by tools like
`@sentry/nextjs`.

## Install

```sh
yarn add -D @wvb/electron-builder
```

## Usage

Wrap your electron-builder config with `withWebViewBundle`. Settings are read from your
webview-bundle config (`wvb.config.ts`), so that file stays the single source of truth.

```ts
// electron-builder.config.ts
import { withWebViewBundle } from '@wvb/electron-builder';

export default withWebViewBundle({
  appId: 'com.example.app',
  asar: true,
  mac: { target: 'dmg' },
  win: { target: 'nsis' },
  linux: { target: 'AppImage' },
});
```

```ts
// wvb.config.ts
import { defineConfig } from '@wvb/cli';

export default defineConfig({
  remote: { endpoint: 'https://cdn.example.com' },
  builtin: {
    // Install everything published to the remote...
    target: { type: 'remote' },
    // ...or pack local workspaces instead:
    // target: { type: 'local', workspaces: ['packages/*'] },
  },
});
```

The HoC injects an electron-builder [`afterPack`](https://www.electron.build/configuration/configuration#afterpack)
hook. It runs **once per platform/arch target**, after the app is packed (so bundles land next to
`app.asar`, outside the archive) and **before** code signing (so they are signed/notarized along
with the app). It does not run for `electron .` dev sessions, so development stays bundle-free.

An existing `afterPack` **function** in your config is preserved — your hook runs first, then the
bundle install. (electron-builder also allows `afterPack` to be a module-path string; the HoC can't
faithfully resolve that, so it throws instead of silently dropping it — convert it to a function, or
call `webViewBundleAfterPack()` from your own hook module.)

### Without the HoC

The HoC transforms a config object, so it only works with a JS/TS electron-builder config file. If
your build is driven by `electron-builder.yml` or the `package.json` `build` field, reference the
hook by module path instead:

```js
// wvb-after-pack.cjs
module.exports = require('@wvb/electron-builder').webViewBundleAfterPack();
```

```yaml
# electron-builder.yml
afterPack: ./wvb-after-pack.cjs
```

## Options

`withWebViewBundle(config, options?)` and `webViewBundleAfterPack(options?)` take the same options.
All are optional; by default everything is read from `wvb.config.ts`.

| Option                   | Type                 | Default     | Description                                                                                                                        |
| ------------------------ | -------------------- | ----------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `builtin`                | `BuiltinConfig`      | from config | Inline `builtin` config. Overrides the `builtin` block in `wvb.config.ts` (inline wins).                                          |
| `root`                   | `string`             | `cwd`       | Project root used to locate `wvb.config.ts` and resolve relative paths.                                                           |
| `configFile`             | `string \| boolean`  | `true`      | `true`: auto-discover a config file. `string`: explicit path. `false`: inline options only.                                       |
| `channel`                | `string`             | —           | Release channel to install from (e.g. `"beta"`). Remote target only.                                                             |
| `bundlesDir`             | `string`             | `'bundles'` | Sub-directory of the app's `Resources` to place bundles in. Must match `@wvb/electron`'s runtime `builtinDir` basename.           |
| `throwWhenBuiltinIsEmpty`| `boolean`            | `true`      | Fail the build when no bundles are installed. Set to `false` to allow a build with no builtin bundles.                            |

## How it works

`@wvb/electron`'s runtime resolves builtin bundles from `<process.resourcesPath>/bundles` in a
packaged app. This integration stages the bundles with `@wvb/cli`'s `builtin()` API, then copies the
staged tree (`manifest.json` + `<name>/<name>_<version>.wvb`) into:

- **macOS** — `<appOutDir>/<ProductName>.app/Contents/Resources/bundles`
- **Windows / Linux** — `<appOutDir>/resources/bundles`

Bundles are placed in `Resources` **outside** `app.asar`, so no `asarUnpack` configuration is
required.

> Note: there is no cross-target download cache, so a multi-target build (e.g. `-mwl`) installs the
> bundles once per target. Each install cleans its own staging directory, so the targets never
> interfere.

## License

MIT
