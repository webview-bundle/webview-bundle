import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import type { Bundle } from '@wvb/node';
import { readBundle } from '@wvb/node';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { pathExists } from '../fs.js';
import { pack } from './pack.js';

let root: string;

beforeEach(async () => {
  root = await fs.mkdtemp(path.join(os.tmpdir(), 'wvb-cli-pack-'));
});

afterEach(async () => {
  await fs.rm(root, { recursive: true, force: true });
});

async function write(rel: string, content: string) {
  const file = path.join(root, rel);
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, content);
}

function entryPaths(bundle: Bundle): string[] {
  return Object.keys(bundle.descriptor().index().entries()).sort();
}

function readText(bundle: Bundle, entryPath: string): string {
  return bundle.getData(entryPath)!.toString('utf8');
}

describe('pack', () => {
  it('packs every file under srcDir as an entry with a leading slash', async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('src/assets/app.js', 'console.log("app");');

    const { bundle } = await pack({ srcDir: './src', outFile: './app', cwd: root });

    expect(entryPaths(bundle)).toEqual(['/assets/app.js', '/index.html']);
    expect(readText(bundle, '/index.html')).toBe('<h1>Hello</h1>');
    expect(readText(bundle, '/assets/app.js')).toBe('console.log("app");');
  });

  it('includes dotfiles', async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('src/.env', 'TOKEN=1');

    const { bundle } = await pack({ srcDir: './src', outFile: './app', cwd: root });

    expect(entryPaths(bundle)).toEqual(['/.env', '/index.html']);
  });

  it('detects the content type of each entry from its file extension', async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('src/style.css', 'body { margin: 0 }');

    const { bundle } = await pack({ srcDir: './src', outFile: './app', cwd: root });
    const entries = bundle.descriptor().index().entries();

    expect(entries['/index.html']?.contentType).toBe('text/html');
    expect(entries['/style.css']?.contentType).toBe('text/css');
  });

  it('appends the ".wvb" extension to the out file when omitted', async () => {
    await write('src/index.html', '<h1>Hello</h1>');

    const { outFilePath } = await pack({ srcDir: './src', outFile: './dist/app', cwd: root });

    expect(outFilePath).toBe(path.join(root, 'dist', 'app.wvb'));
  });

  it('keeps a single ".wvb" extension when the out file already has one', async () => {
    await write('src/index.html', '<h1>Hello</h1>');

    const { outFilePath } = await pack({ srcDir: './src', outFile: './dist/app.wvb', cwd: root });

    expect(outFilePath).toBe(path.join(root, 'dist', 'app.wvb'));
  });

  it('resolves srcDir and outFile relative to cwd', async () => {
    await write('nested/src/index.html', '<h1>Hello</h1>');

    const { outFilePath, bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: path.join(root, 'nested'),
    });

    expect(outFilePath).toBe(path.join(root, 'nested', 'app.wvb'));
    expect(entryPaths(bundle)).toEqual(['/index.html']);
  });

  it('uses an absolute outFile as-is', async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    const absoluteOutFile = path.join(root, 'somewhere', 'app.wvb');

    const { outFilePath } = await pack({ srcDir: './src', outFile: absoluteOutFile, cwd: root });

    expect(outFilePath).toBe(absoluteOutFile);
  });

  it('writes a readable archive, creating missing parent directories', async () => {
    await write('src/index.html', '<h1>Hello</h1>');

    const { outFilePath } = await pack({
      srcDir: './src',
      outFile: './deep/nested/app',
      cwd: root,
    });

    const written = await readBundle(outFilePath);
    expect(entryPaths(written)).toEqual(['/index.html']);
    expect(readText(written, '/index.html')).toBe('<h1>Hello</h1>');
  });

  it('does not touch the disk when write is false', async () => {
    await write('src/index.html', '<h1>Hello</h1>');

    const { outFilePath, bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      write: false,
    });

    expect(entryPaths(bundle)).toEqual(['/index.html']);
    expect(await pathExists(outFilePath)).toBe(false);
  });

  it('overwrites an existing archive by default', async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('app.wvb', 'this is not a bundle');

    const { outFilePath } = await pack({ srcDir: './src', outFile: './app', cwd: root });

    const written = await readBundle(outFilePath);
    expect(entryPaths(written)).toEqual(['/index.html']);
  });

  it('throws when the archive already exists and overwrite is false', async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('app.wvb', 'keep me');

    await expect(
      pack({ srcDir: './src', outFile: './app', cwd: root, overwrite: false })
    ).rejects.toThrow('Outfile already exists: app.wvb');

    expect(await fs.readFile(path.join(root, 'app.wvb'), 'utf8')).toBe('keep me');
  });

  it('throws when srcDir has no files', async () => {
    await fs.mkdir(path.join(root, 'src'));

    await expect(pack({ srcDir: './src', outFile: './app', cwd: root })).rejects.toThrow(
      'No files to pack bundle'
    );
  });

  it('throws when srcDir does not exist', async () => {
    await expect(pack({ srcDir: './missing', outFile: './app', cwd: root })).rejects.toThrow(
      'No files to pack bundle'
    );
  });

  it('throws when every file is ignored', async () => {
    await write('src/index.html', '<h1>Hello</h1>');

    await expect(
      pack({ srcDir: './src', outFile: './app', cwd: root, ignores: [['**/*']] })
    ).rejects.toThrow('No files to pack bundle');
  });
});

describe('pack ignores', () => {
  beforeEach(async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('src/app.js', 'console.log("app");');
    await write('src/app.js.map', '{"version":3}');
  });

  it('excludes files matching a glob pattern', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      ignores: [['**/*.map']],
    });

    expect(entryPaths(bundle)).toEqual(['/app.js', '/index.html']);
  });

  it('excludes files matching a regular expression', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      ignores: [[/\.map$/]],
    });

    expect(entryPaths(bundle)).toEqual(['/app.js', '/index.html']);
  });

  it('excludes files rejected by a predicate that receives the srcDir-relative path', async () => {
    const seen: string[] = [];

    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      ignores: [
        file => {
          seen.push(file);
          return file.endsWith('.map');
        },
      ],
    });

    expect(entryPaths(bundle)).toEqual(['/app.js', '/index.html']);
    expect(seen.sort()).toEqual(['app.js', 'app.js.map', 'index.html']);
  });

  it('awaits an async predicate', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      ignores: [async file => file.endsWith('.map')],
    });

    expect(entryPaths(bundle)).toEqual(['/app.js', '/index.html']);
  });

  it('applies every pattern config in the list', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      ignores: [['**/*.map'], ['**/*.js']],
    });

    expect(entryPaths(bundle)).toEqual(['/index.html']);
  });

  it('keeps every file when the ignore list is empty', async () => {
    const { bundle } = await pack({ srcDir: './src', outFile: './app', cwd: root, ignores: [] });

    expect(entryPaths(bundle)).toEqual(['/app.js', '/app.js.map', '/index.html']);
  });
});

describe('pack headers', () => {
  beforeEach(async () => {
    await write('src/index.html', '<h1>Hello</h1>');
    await write('src/app.js', 'console.log("app");');
  });

  function headersOf(bundle: Bundle, entryPath: string): Record<string, string> {
    return bundle.descriptor().index().getEntry(entryPath)!.headers;
  }

  it('assigns no headers when no header config is given', async () => {
    const { bundle } = await pack({ srcDir: './src', outFile: './app', cwd: root });

    expect(headersOf(bundle, '/index.html')).toEqual({});
  });

  it('assigns headers from a pattern-keyed record', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      headers: [
        {
          '*.html': { 'cache-control': 'max-age=0' },
          '*.js': { 'cache-control': 'max-age=31536000' },
        },
      ],
    });

    expect(headersOf(bundle, '/index.html')).toEqual({ 'cache-control': 'max-age=0' });
    expect(headersOf(bundle, '/app.js')).toEqual({ 'cache-control': 'max-age=31536000' });
  });

  it('assigns headers from an array of pattern/headers tuples', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      headers: [[['*.html', [['x-frame-options', 'DENY']]]]],
    });

    expect(headersOf(bundle, '/index.html')).toEqual({ 'x-frame-options': 'DENY' });
    expect(headersOf(bundle, '/app.js')).toEqual({});
  });

  it('lowercases header names', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      headers: [{ '*.html': { 'Cache-Control': 'no-store' } }],
    });

    expect(headersOf(bundle, '/index.html')).toEqual({ 'cache-control': 'no-store' });
  });

  it('assigns headers returned from a function that receives the srcDir-relative path', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      headers: [file => (file === 'index.html' ? { 'x-source': file } : null)],
    });

    expect(headersOf(bundle, '/index.html')).toEqual({ 'x-source': 'index.html' });
    expect(headersOf(bundle, '/app.js')).toEqual({});
  });

  it('lets a later config override the same header name', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      headers: [
        { '*.html': { 'cache-control': 'max-age=0' } },
        { '*': { 'cache-control': 'no-store' } },
      ],
    });

    expect(headersOf(bundle, '/index.html')).toEqual({ 'cache-control': 'no-store' });
  });

  it('merges headers from configs matching different patterns', async () => {
    const { bundle } = await pack({
      srcDir: './src',
      outFile: './app',
      cwd: root,
      headers: [{ '*.html': { 'x-a': '1' } }, { '*': { 'x-b': '2' } }],
    });

    expect(headersOf(bundle, '/index.html')).toEqual({ 'x-a': '1', 'x-b': '2' });
  });
});
