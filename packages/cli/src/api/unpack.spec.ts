import { Buffer } from 'node:buffer';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { BundleBuilder, writeBundle } from '@wvb/node';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { pathExists } from '../fs.js';
import { unpack } from './unpack.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-unpack-'));
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

async function writeBundleFile(rel: string, entries: Record<string, string>) {
  const builder = new BundleBuilder();
  for (const [entryPath, content] of Object.entries(entries)) {
    builder.insertEntry(entryPath, Buffer.from(content, 'utf8'));
  }
  const file = path.join(root, rel);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await writeBundle(builder.build(), file);
  return file;
}

function readText(rel: string) {
  return fs.readFile(path.join(root, rel), 'utf8');
}

describe('unpack', () => {
  it('extracts every entry into ".wvb/<bundle file name>" by default', async () => {
    await writeBundleFile('app.wvb', {
      '/index.html': '<h1>Hello</h1>',
      '/app.js': 'console.log("app");',
    });

    await unpack({ file: './app.wvb', cwd: root });

    expect(await readText('.wvb/app/index.html')).toBe('<h1>Hello</h1>');
    expect(await readText('.wvb/app/app.js')).toBe('console.log("app");');
  });

  it('extracts into an explicit outDir', async () => {
    await writeBundleFile('app.wvb', { '/index.html': '<h1>Hello</h1>' });

    await unpack({ file: './app.wvb', outDir: './extracted', cwd: root });

    expect(await readText('extracted/index.html')).toBe('<h1>Hello</h1>');
  });

  it('recreates nested directories for nested entries', async () => {
    await writeBundleFile('app.wvb', { '/assets/js/app.js': 'console.log("app");' });

    await unpack({ file: './app.wvb', outDir: './extracted', cwd: root });

    expect(await readText('extracted/assets/js/app.js')).toBe('console.log("app");');
  });

  it('appends the ".wvb" extension to the file argument when omitted', async () => {
    await writeBundleFile('app.wvb', { '/index.html': '<h1>Hello</h1>' });

    await unpack({ file: './app', outDir: './extracted', cwd: root });

    expect(await readText('extracted/index.html')).toBe('<h1>Hello</h1>');
  });

  it('accepts an absolute file path', async () => {
    const file = await writeBundleFile('nested/app.wvb', { '/index.html': '<h1>Hello</h1>' });

    await unpack({ file, outDir: './extracted', cwd: root });

    expect(await readText('extracted/index.html')).toBe('<h1>Hello</h1>');
  });

  it('returns the parsed bundle', async () => {
    await writeBundleFile('app.wvb', { '/index.html': '<h1>Hello</h1>' });

    const bundle = await unpack({ file: './app.wvb', outDir: './extracted', cwd: root });

    expect(bundle.descriptor().header().version()).toBe('v1');
    expect(Object.keys(bundle.descriptor().index().entries())).toEqual(['/index.html']);
  });

  it('does not write anything when write is false', async () => {
    await writeBundleFile('app.wvb', { '/index.html': '<h1>Hello</h1>' });

    const bundle = await unpack({ file: './app.wvb', cwd: root, write: false });

    expect(Object.keys(bundle.descriptor().index().entries())).toEqual(['/index.html']);
    expect(await pathExists(path.join(root, '.wvb'))).toBe(false);
  });

  it('throws when the file does not exist', async () => {
    await expect(unpack({ file: './missing', cwd: root })).rejects.toThrow(
      'File does not exist: missing.wvb'
    );
  });

  it('throws when the output directory already exists', async () => {
    await writeBundleFile('app.wvb', { '/index.html': '<h1>Hello</h1>' });
    await fs.mkdir(path.join(root, 'extracted'), { recursive: true });
    await fs.writeFile(path.join(root, 'extracted', 'stale.txt'), 'stale');

    await expect(unpack({ file: './app.wvb', outDir: './extracted', cwd: root })).rejects.toThrow(
      'Output directory already exists: extracted'
    );

    expect(await readText('extracted/stale.txt')).toBe('stale');
  });

  it('replaces the output directory when clean is true', async () => {
    await writeBundleFile('app.wvb', { '/index.html': '<h1>Hello</h1>' });
    await fs.mkdir(path.join(root, 'extracted'), { recursive: true });
    await fs.writeFile(path.join(root, 'extracted', 'stale.txt'), 'stale');

    await unpack({ file: './app.wvb', outDir: './extracted', cwd: root, clean: true });

    expect(await fs.readdir(path.join(root, 'extracted'))).toEqual(['index.html']);
  });
});
