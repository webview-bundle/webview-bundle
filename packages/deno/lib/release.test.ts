import { assertEquals } from '@std/assert';
import {
  osOfTarget,
  parseChecksums,
  releaseAssetName,
  releaseAssetSuffixes,
  releaseBaseUrl,
  SUPPORTED_TARGETS,
  sha256Hex,
} from './release.ts';

Deno.test('osOfTarget classifies the supported triples', () => {
  assertEquals(osOfTarget('aarch64-apple-darwin'), 'darwin');
  assertEquals(osOfTarget('x86_64-apple-darwin'), 'darwin');
  assertEquals(osOfTarget('aarch64-unknown-linux-gnu'), 'linux');
  assertEquals(osOfTarget('x86_64-unknown-linux-gnu'), 'linux');
  assertEquals(osOfTarget('x86_64-pc-windows-msvc'), 'windows');
});

Deno.test('releaseAssetName follow the per-os prefix + extension', () => {
  assertEquals(releaseAssetName('aarch64-apple-darwin'), 'libwvb_deno-aarch64-apple-darwin.dylib');
  assertEquals(
    releaseAssetName('x86_64-unknown-linux-gnu'),
    'libwvb_deno-x86_64-unknown-linux-gnu.so'
  );
  assertEquals(releaseAssetName('x86_64-pc-windows-msvc'), 'wvb_deno-x86_64-pc-windows-msvc.dll');
});

Deno.test('releaseBaseUrl uses the slash-separated `deno/<version>` tag', () => {
  assertEquals(
    releaseBaseUrl('0.1.0'),
    'https://github.com/webview-bundle/webview-bundle/releases/download/deno/0.1.0'
  );
});

Deno.test('releaseAssetSuffixes maps every supported target by os → arch', () => {
  const suffixes = releaseAssetSuffixes();
  // The suffix plug appends must reconstruct exactly the asset name for each target.
  for (const target of SUPPORTED_TARGETS) {
    const os = osOfTarget(target);
    const arch = target.split('-')[0]!;
    assertEquals(suffixes[os]?.[arch], `-${target}`);
  }
  assertEquals(suffixes.darwin?.aarch64, '-aarch64-apple-darwin');
  assertEquals(suffixes.windows?.x86_64, '-x86_64-pc-windows-msvc');
});

Deno.test('parseChecksums reads sha256sum output (two spaces, binary marker, blank lines)', () => {
  const text = [
    'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  libwvb_deno-aarch64-apple-darwin.dylib',
    '',
    'AABBCCDDEEFF00112233445566778899AABBCCDDEEFF00112233445566778899 *wvb_deno-x86_64-pc-windows-msvc.dll',
  ].join('\n');
  const map = parseChecksums(text);
  assertEquals(
    map.get('libwvb_deno-aarch64-apple-darwin.dylib'),
    'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
  );
  // Hex is normalized to lowercase; the `*` binary marker is stripped from the filename.
  assertEquals(
    map.get('wvb_deno-x86_64-pc-windows-msvc.dll'),
    'aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899'
  );
});

Deno.test('sha256Hex matches the NIST "abc" test vector', async () => {
  const hex = await sha256Hex(new TextEncoder().encode('abc'));
  assertEquals(hex, 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
});
