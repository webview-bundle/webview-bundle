import { describe, expect, it } from 'vitest';
import {
  compareSizes,
  formatBytes,
  mergeReports,
  readZipEntries,
  renderComparison,
  reportMarker,
  type SizeReport,
  sliceReport,
  zipBinarySizes,
} from './size.ts';

/** A minimal stored (uncompressed) zip with the given files, plus a trailing archive comment. */
function makeZip(files: Record<string, number>, comment = ''): Buffer {
  const locals: Buffer[] = [];
  const centrals: Buffer[] = [];
  let offset = 0;
  for (const [name, size] of Object.entries(files)) {
    const nameBuf = Buffer.from(name, 'utf8');
    const data = Buffer.alloc(size);
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt32LE(size, 18);
    local.writeUInt32LE(size, 22);
    local.writeUInt16LE(nameBuf.length, 26);
    locals.push(local, nameBuf, data);
    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt32LE(size, 20);
    central.writeUInt32LE(size, 24);
    central.writeUInt16LE(nameBuf.length, 28);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, nameBuf);
    offset += 30 + nameBuf.length + size;
  }
  const cd = Buffer.concat(centrals);
  const commentBuf = Buffer.from(comment, 'utf8');
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(Object.keys(files).length, 8);
  eocd.writeUInt16LE(Object.keys(files).length, 10);
  eocd.writeUInt32LE(cd.length, 12);
  eocd.writeUInt32LE(offset, 16);
  eocd.writeUInt16LE(commentBuf.length, 20);
  return Buffer.concat([...locals, cd, eocd, commentBuf]);
}

describe('readZipEntries', () => {
  it('lists entries with their uncompressed sizes', () => {
    const zip = makeZip({ 'a/libx.so': 12, 'a/b.kt': 3 }, 'hello');
    expect(readZipEntries(zip)).toEqual([
      { name: 'a/libx.so', size: 12 },
      { name: 'a/b.kt', size: 3 },
    ]);
  });

  it('rejects non-zip data', () => {
    expect(() => readZipEntries(Buffer.alloc(64))).toThrow(/end of central directory/);
  });
});

describe('zipBinarySizes', () => {
  it('keeps only native binaries, keyed under the zip name', () => {
    const zip = makeZip({
      'jniLibs/arm64-v8a/libwvb_ffi.so': 100,
      'ios-arm64/libwvb_ffi.a': 200,
      'kotlin/Wvb.kt': 5,
      'Info.plist': 1,
    });
    expect(zipBinarySizes('android.zip', zip)).toEqual({
      'android.zip/jniLibs/arm64-v8a/libwvb_ffi.so': 100,
      'android.zip/ios-arm64/libwvb_ffi.a': 200,
    });
  });
});

describe('mergeReports / sliceReport', () => {
  const a: SizeReport = { version: 1, packages: { node: { 'x.node': 1 } } };
  const b: SizeReport = { version: 1, packages: { node: { 'y.node': 2 }, deno: { 'z.so': 3 } } };

  it('merges package maps', () => {
    expect(mergeReports([a, b]).packages).toEqual({
      node: { 'x.node': 1, 'y.node': 2 },
      deno: { 'z.so': 3 },
    });
  });

  it('slices one package', () => {
    expect(sliceReport(b, 'deno')).toEqual({ version: 1, packages: { deno: { 'z.so': 3 } } });
    expect(sliceReport(b, 'ffi')).toBeNull();
  });
});

describe('formatBytes', () => {
  it('picks a unit', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2.0 KiB');
    expect(formatBytes(7362896)).toBe('7.02 MiB');
  });
});

describe('compare + render', () => {
  const current: SizeReport = {
    version: 1,
    packages: {
      node: { 'a.node': 1100, 'b.node': 900, 'c.node': 50 },
      ffi: { 'android.zip/x86_64/libwvb_ffi.so': 1000 },
    },
  };
  const comparisons = compareSizes(
    current,
    new Map([
      ['node', { tag: 'node/0.3.1', sizes: { 'a.node': 1000, 'b.node': 1000 } }],
      ['ffi', null],
    ])
  );

  it('pairs current sizes with the baseline', () => {
    expect(comparisons.map(c => c.pkg)).toEqual(['ffi', 'node']);
    expect(comparisons[1]!.rows).toEqual([
      { name: 'a.node', base: 1000, current: 1100 },
      { name: 'b.node', base: 1000, current: 900 },
      { name: 'c.node', base: null, current: 50 },
    ]);
  });

  it('renders a markdown report with a marker and flags', () => {
    const md = renderComparison(comparisons, { id: 'native', title: 'Size', threshold: 5 });
    expect(md.startsWith(reportMarker('native'))).toBe(true);
    expect(md).toContain('`node/0.3.1`');
    expect(md).toContain('`ffi` (no release)');
    expect(md).toContain('| `a.node` | 1000 B | 1.1 KiB | +100 B | +10.00% ⚠️ |');
    expect(md).toContain('| `b.node` | 1000 B | 900 B | -100 B | -10.00% 🔻 |');
    expect(md).toContain('| `c.node` | - | 50 B | new |  |');
    expect(md).toContain('1 binary grew by 5% or more.');
  });
});
