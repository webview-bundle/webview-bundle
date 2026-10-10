import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { planFiles, type RenderContext, SUBSTITUTABLE } from './render.js';
import { loadManifest, type Template, templatesDir } from './templates.js';
import { REGISTRIES } from './versions.js';

const dir = templatesDir();
const manifest = await loadManifest(dir);

// A fixed map so rendering tests never touch the network; every registry package gets a plausible
// released version.
const versions: Record<string, string> = Object.fromEntries(
  Object.keys(REGISTRIES).map(pkg => [pkg, '1.0.0'])
);

async function walk(root: string, prefix = ''): Promise<string[]> {
  const entries = await fs.readdir(root, { withFileTypes: true });
  const files: string[] = [];
  for (const entry of entries) {
    const rel = prefix === '' ? entry.name : `${prefix}/${entry.name}`;
    if (entry.isDirectory()) {
      files.push(...(await walk(path.join(root, entry.name), rel)));
    } else {
      files.push(rel);
    }
  }
  return files;
}

const allFiles = await walk(dir);

function contextFor(): RenderContext {
  return { projectName: 'demo-app', bundleName: 'demo-app', pm: 'npm', pmRun: 'npm run', versions };
}

describe('templates directory', () => {
  // A real package.json under templates/ turns the template into a yarn workspace AND makes
  // `yarn pack` silently drop the whole templates/ tree from the tarball.
  it('contains no real package.json — manifests must be named _package.json', () => {
    expect(allFiles.filter(f => path.basename(f) === 'package.json')).toEqual([]);
  });

  // `yarn pack` strips real .gitignore files, so templates ship them as _gitignore.
  it('contains no real .gitignore — it must be named _gitignore', () => {
    expect(allFiles.filter(f => path.basename(f) === '.gitignore')).toEqual([]);
  });

  it('keeps every .json template parseable, proving tokens sit inside string values', async () => {
    const jsonFiles = allFiles.filter(f => f.endsWith('.json'));
    expect(jsonFiles.length).toBeGreaterThan(0);
    for (const file of jsonFiles) {
      const raw = await fs.readFile(path.join(dir, file), 'utf8');
      expect(() => JSON.parse(raw), `${file} is not valid JSON with tokens in place`).not.toThrow();
    }
  });

  it('references only packages that have a configured registry', async () => {
    const token = /\{\{\s*wvbVersion\s*:\s*([^}]+?)\s*\}\}/g;
    for (const file of allFiles) {
      const raw = await fs.readFile(path.join(dir, file), 'utf8');
      for (const match of raw.matchAll(token)) {
        expect(
          REGISTRIES[match[1] as string],
          `${file} references ${match[1]}, which has no registry in versions.ts`
        ).toBeDefined();
      }
    }
  });

  // A token in a file the engine will not substitute (e.g. a .properties file) would ship literally.
  it('places tokens only in files the engine substitutes', async () => {
    const binaryLike = new Set(['.png', '.jar', '.keystore', '.ico', '.icns', '.webp', '.jpg']);
    for (const file of allFiles) {
      const ext = path.extname(file);
      if (SUBSTITUTABLE.has(ext) || binaryLike.has(ext)) {
        continue;
      }
      const raw = await fs.readFile(path.join(dir, file), 'utf8');
      expect(
        raw.includes('{{'),
        `${file} has a token but its extension is not substituted by the engine`
      ).toBe(false);
    }
  });

  it('never writes a literal @wvb/* range where a version token belongs', async () => {
    const literal = /"(@wvb\/[a-z-]+)"\s*:\s*"(?!\{\{)([^"]+)"/g;
    for (const file of allFiles.filter(f => path.basename(f) === '_package.json')) {
      const raw = await fs.readFile(path.join(dir, file), 'utf8');
      const offenders = [...raw.matchAll(literal)].map(m => `${m[1]}: ${m[2]}`);
      expect(offenders, `${file} pins @wvb/* literally instead of using {{wvbVersion:…}}`).toEqual(
        []
      );
    }
  });
});

describe('templates.json', () => {
  it('is not empty', () => {
    expect(Object.keys(manifest).length).toBeGreaterThan(0);
  });

  it('points every layer at a real directory', async () => {
    for (const [id, template] of Object.entries(manifest)) {
      for (const layer of template.layers) {
        const stat = await fs.stat(path.join(dir, layer)).catch(() => null);
        expect(stat?.isDirectory(), `template "${id}" references missing layer "${layer}"`).toBe(
          true
        );
      }
    }
  });

  it('gives every experimental template caveats to show the user', () => {
    for (const [id, template] of Object.entries(manifest)) {
      if (template.status === 'experimental') {
        expect(
          (template.caveats ?? []).length,
          `experimental template "${id}" has no caveats`
        ).toBeGreaterThan(0);
      }
    }
  });
});

describe('rendering every template', () => {
  it('produces a package.json and .gitignore with no unresolved tokens', async () => {
    for (const [id, template] of Object.entries(manifest)) {
      const writes = await planFiles(dir, template.layers, contextFor());
      const paths = writes.map(w => w.path);
      const label = id;

      expect(paths, `${label} produced no package.json`).toContain('package.json');
      expect(paths, `${label} produced no .gitignore`).toContain('.gitignore');

      // Only substitutable files pass through token replacement; binaries are copied raw and may
      // legitimately contain the token bytes.
      for (const write of writes.filter(w => SUBSTITUTABLE.has(path.extname(w.path)))) {
        const text = write.contents.toString('utf8');
        expect(text.includes('{{'), `${label}: ${write.path} has an unresolved token`).toBe(false);
      }

      const manifestWrite = writes.find(w => w.path === 'package.json');
      if (manifestWrite != null) {
        expect(
          () => JSON.parse(manifestWrite.contents.toString('utf8')),
          `${label}: invalid package.json`
        ).not.toThrow();
      }
    }
  });

  it('deep-merges nested _package.json across layers, not only the root', async () => {
    const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'create-wvb-layers-'));
    try {
      await fs.mkdir(path.join(tmp, 'base', 'web'), { recursive: true });
      await fs.mkdir(path.join(tmp, 'overlay', 'web'), { recursive: true });
      await fs.writeFile(
        path.join(tmp, 'base', 'web', '_package.json'),
        JSON.stringify({ name: 'demo', private: true, dependencies: { '@wvb/bridge': '^0.1.0' } })
      );
      await fs.writeFile(
        path.join(tmp, 'overlay', 'web', '_package.json'),
        JSON.stringify({ devDependencies: { vitest: '^4.0.0' } })
      );
      const writes = await planFiles(tmp, ['base', 'overlay'], contextFor());
      const web = writes.find(w => w.path === 'web/package.json');
      expect(web, 'nested manifest was not emitted as web/package.json').toBeDefined();
      const merged = JSON.parse((web as { contents: Buffer }).contents.toString('utf8'));
      expect(merged).toMatchObject({
        name: 'demo',
        private: true,
        dependencies: { '@wvb/bridge': '^0.1.0' },
        devDependencies: { vitest: '^4.0.0' },
      });
    } finally {
      await fs.rm(tmp, { recursive: true, force: true });
    }
  });

  it('writes files to disk under --dry-run only when asked', async () => {
    const tmp = await fs.mkdtemp(path.join(os.tmpdir(), 'create-wvb-'));
    try {
      const entry = Object.entries(manifest)[0] as [string, Template];
      const writes = await planFiles(dir, entry[1].layers, contextFor());
      const { materialize } = await import('./render.js');
      await materialize(writes, tmp, { dryRun: true });
      expect(await fs.readdir(tmp)).toEqual([]);
      await materialize(writes, tmp);
      expect((await fs.readdir(tmp)).length).toBeGreaterThan(0);
    } finally {
      await fs.rm(tmp, { recursive: true, force: true });
    }
  });
});
