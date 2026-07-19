import fs from 'node:fs/promises';
import path from 'node:path';
import {
  type AfterPackContext,
  resolveResourcesPath,
  withWebviewBundle,
} from '@wvb/electron-builder';

/**
 * Copy the native `@wvb/node` addon into the packaged app.
 *
 * In this monorepo `@wvb/node`'s platform sub-packages are workspace symlinks (`packages/node/npm/*`)
 * that resolve outside the app dir, which electron-builder's dependency collection rejects. So
 * `@wvb/electron` (which pulls `@wvb/node`) is a **devDependency** — Vite bundles it into the main
 * process, so it isn't needed at runtime — and electron-builder collects no production dependencies.
 * That leaves only the one runtime-required native addon, `@wvb/node`, which we copy in here. It is
 * resolved exactly as `@wvb/electron` sees it (the npm copy, not the workspace one). A standalone
 * consumer keeps `@wvb/electron` as a normal dependency and doesn't need any of this.
 */
async function copyWvbNode(context: AfterPackContext): Promise<void> {
  const { createRequire } = await import('node:module');
  const base = createRequire(path.join(process.cwd(), 'noop.js'));
  const electronPkg = base.resolve('@wvb/electron/package.json');
  const nodePkg = createRequire(electronPkg).resolve('@wvb/node/package.json');
  const dest = path.join(resolveResourcesPath(context), 'app', 'node_modules', '@wvb', 'node');
  await fs.rm(dest, { recursive: true, force: true });
  await fs.cp(path.dirname(nodePkg), dest, { recursive: true, dereference: true });
}

export default withWebviewBundle(
  {
    appId: 'dev.wvb.playground.electron-builder',
    productName: 'WebviewBundlePlaygroundElectronBuilder',
    // asar is disabled so the `@wvb/node` addon copied in `afterPack` (which cannot be a collected
    // dependency here — see `copyWvbNode`) is loadable straight from `Resources/app/node_modules`.
    asar: false,
    directories: {
      output: 'out',
    },
    mac: { target: 'dir' },
    linux: { target: 'dir' },
    win: { target: 'dir' },
    afterPack: copyWvbNode,
  },
  // Install builtin bundles from the `remote.endpoint` resolved from wvb.config.ts at package time.
  { builtin: {} }
);
