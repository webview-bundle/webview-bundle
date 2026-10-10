import { defineConfig } from '@wvb/config';

export default defineConfig({
  builtin: {
    /**
     * Must stay './bundles': unpackaged, `@wvb/electron` reads builtin bundles from
     * `process.cwd()/bundles`. The package default ('.wvb/builtin/bundles') would make
     * `npm start` find zero bundles.
     */
    outDir: './bundles',
    target: {
      type: 'local',
      workspaces: ['./web'],
    },
  },
});
