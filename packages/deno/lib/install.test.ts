import { assertEquals } from '@std/assert';
import { localFileName, releaseAssetName } from './install.ts';

Deno.test('releaseAssetName maps target triples to per-OS asset names', () => {
  assertEquals(releaseAssetName('aarch64-apple-darwin'), 'libwvb_deno-aarch64-apple-darwin.dylib');
  assertEquals(releaseAssetName('x86_64-apple-darwin'), 'libwvb_deno-x86_64-apple-darwin.dylib');
  assertEquals(
    releaseAssetName('x86_64-unknown-linux-gnu'),
    'libwvb_deno-x86_64-unknown-linux-gnu.so'
  );
  assertEquals(releaseAssetName('x86_64-pc-windows-msvc'), 'wvb_deno-x86_64-pc-windows-msvc.dll');
});

Deno.test('localFileName is the plain platform filename', () => {
  assertEquals(localFileName('aarch64-apple-darwin'), 'libwvb_deno.dylib');
  assertEquals(localFileName('x86_64-unknown-linux-musl'), 'libwvb_deno.so');
  assertEquals(localFileName('x86_64-pc-windows-msvc'), 'wvb_deno.dll');
});
